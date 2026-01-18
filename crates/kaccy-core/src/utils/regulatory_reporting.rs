//! Regulatory Reporting System
//!
//! This module provides comprehensive regulatory reporting capabilities including:
//! - Trade reporting (EMIR, MiFID II compliant structures)
//! - Transaction reporting
//! - Position reporting
//! - Best execution reporting

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Regulatory regime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryRegime {
    /// European Market Infrastructure Regulation
    EMIR,
    /// Markets in Financial Instruments Directive II
    MiFIDII,
    /// Dodd-Frank (US)
    DoddFrank,
    /// Securities and Exchange Commission (US)
    SEC,
    /// Financial Conduct Authority (UK)
    FCA,
    /// Generic/Custom reporting
    Custom,
}

/// Trade report for regulatory submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeReport {
    /// Unique trade identifier
    pub trade_id: String,
    /// Reporting regime
    pub regime: RegulatoryRegime,
    /// Trade timestamp
    pub trade_timestamp: DateTime<Utc>,
    /// Instrument identifier (ISIN, etc.)
    pub instrument_id: String,
    /// Instrument type
    pub instrument_type: InstrumentType,
    /// Trade quantity
    pub quantity: Decimal,
    /// Trade price
    pub price: Decimal,
    /// Trade currency
    pub currency: String,
    /// Buyer identifier
    pub buyer_id: String,
    /// Seller identifier
    pub seller_id: String,
    /// Venue identifier
    pub venue_id: String,
    /// Trade type
    pub trade_type: TradeType,
    /// Settlement date
    pub settlement_date: DateTime<Utc>,
    /// Additional fields
    pub additional_data: HashMap<String, String>,
}

/// Instrument type for regulatory classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentType {
    Equity,
    Bond,
    Derivative,
    Commodity,
    Forex,
    Crypto,
    Other,
}

/// Trade type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Buy,
    Sell,
    Exchange,
    Transfer,
}

/// Transaction report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReport {
    /// Transaction identifier
    pub transaction_id: String,
    /// Transaction timestamp
    pub timestamp: DateTime<Utc>,
    /// Account identifier
    pub account_id: String,
    /// Transaction type
    pub transaction_type: TransactionType,
    /// Amount
    pub amount: Decimal,
    /// Currency
    pub currency: String,
    /// From account/wallet
    pub from_account: Option<String>,
    /// To account/wallet
    pub to_account: Option<String>,
    /// Transaction status
    pub status: TransactionStatus,
    /// Fees paid
    pub fees: Option<Decimal>,
    /// Reference data
    pub reference: Option<String>,
}

/// Transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Trade,
    Transfer,
    Fee,
    Reward,
    Other,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
}

/// Position report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionReport {
    /// Report date
    pub report_date: DateTime<Utc>,
    /// Account identifier
    pub account_id: String,
    /// Instrument identifier
    pub instrument_id: String,
    /// Position quantity (positive for long, negative for short)
    pub quantity: Decimal,
    /// Average entry price
    pub average_price: Decimal,
    /// Current market price
    pub market_price: Decimal,
    /// Unrealized P&L
    pub unrealized_pnl: Decimal,
    /// Currency
    pub currency: String,
    /// Position type
    pub position_type: PositionType,
}

/// Position type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionType {
    Long,
    Short,
    Neutral,
}

/// Best execution report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestExecutionReport {
    /// Reporting period start
    pub period_start: DateTime<Utc>,
    /// Reporting period end
    pub period_end: DateTime<Utc>,
    /// Order identifier
    pub order_id: String,
    /// Client identifier
    pub client_id: String,
    /// Instrument identifier
    pub instrument_id: String,
    /// Order timestamp
    pub order_timestamp: DateTime<Utc>,
    /// Execution timestamp
    pub execution_timestamp: Option<DateTime<Utc>>,
    /// Requested quantity
    pub requested_quantity: Decimal,
    /// Executed quantity
    pub executed_quantity: Decimal,
    /// Requested price (if limit order)
    pub requested_price: Option<Decimal>,
    /// Executed price
    pub executed_price: Option<Decimal>,
    /// Execution venues considered
    pub venues_considered: Vec<String>,
    /// Selected venue
    pub selected_venue: String,
    /// Execution quality metrics
    pub quality_metrics: ExecutionQualityMetrics,
    /// Justification for venue selection
    pub justification: String,
}

/// Execution quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionQualityMetrics {
    /// Price improvement vs benchmark
    pub price_improvement: Option<Decimal>,
    /// Slippage
    pub slippage: Option<Decimal>,
    /// Fill rate (percentage)
    pub fill_rate: Decimal,
    /// Speed of execution (milliseconds)
    pub execution_speed_ms: u64,
    /// Likelihood of execution score
    pub execution_likelihood: f64,
}

/// Regulatory report generator
pub struct RegulatoryReportGenerator;

impl RegulatoryReportGenerator {
    /// Generate trade report for a given regime
    pub fn generate_trade_report(
        trade_id: impl Into<String>,
        regime: RegulatoryRegime,
    ) -> TradeReport {
        TradeReport {
            trade_id: trade_id.into(),
            regime,
            trade_timestamp: Utc::now(),
            instrument_id: String::new(),
            instrument_type: InstrumentType::Other,
            quantity: Decimal::ZERO,
            price: Decimal::ZERO,
            currency: "USD".to_string(),
            buyer_id: String::new(),
            seller_id: String::new(),
            venue_id: String::new(),
            trade_type: TradeType::Buy,
            settlement_date: Utc::now(),
            additional_data: HashMap::new(),
        }
    }

    /// Generate transaction report
    pub fn generate_transaction_report(transaction_id: impl Into<String>) -> TransactionReport {
        TransactionReport {
            transaction_id: transaction_id.into(),
            timestamp: Utc::now(),
            account_id: String::new(),
            transaction_type: TransactionType::Trade,
            amount: Decimal::ZERO,
            currency: "USD".to_string(),
            from_account: None,
            to_account: None,
            status: TransactionStatus::Completed,
            fees: None,
            reference: None,
        }
    }

    /// Generate position report
    pub fn generate_position_report(
        account_id: impl Into<String>,
        instrument_id: impl Into<String>,
    ) -> PositionReport {
        PositionReport {
            report_date: Utc::now(),
            account_id: account_id.into(),
            instrument_id: instrument_id.into(),
            quantity: Decimal::ZERO,
            average_price: Decimal::ZERO,
            market_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            currency: "USD".to_string(),
            position_type: PositionType::Neutral,
        }
    }

    /// Generate best execution report
    pub fn generate_best_execution_report(
        order_id: impl Into<String>,
        client_id: impl Into<String>,
    ) -> BestExecutionReport {
        BestExecutionReport {
            period_start: Utc::now(),
            period_end: Utc::now(),
            order_id: order_id.into(),
            client_id: client_id.into(),
            instrument_id: String::new(),
            order_timestamp: Utc::now(),
            execution_timestamp: None,
            requested_quantity: Decimal::ZERO,
            executed_quantity: Decimal::ZERO,
            requested_price: None,
            executed_price: None,
            venues_considered: Vec::new(),
            selected_venue: String::new(),
            quality_metrics: ExecutionQualityMetrics {
                price_improvement: None,
                slippage: None,
                fill_rate: Decimal::ZERO,
                execution_speed_ms: 0,
                execution_likelihood: 0.0,
            },
            justification: String::new(),
        }
    }
}

/// Report submission tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSubmission {
    pub submission_id: String,
    pub report_type: ReportType,
    pub regime: RegulatoryRegime,
    pub submission_timestamp: DateTime<Utc>,
    pub status: SubmissionStatus,
    pub acknowledgment_ref: Option<String>,
    pub error_message: Option<String>,
}

/// Report type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportType {
    Trade,
    Transaction,
    Position,
    BestExecution,
}

/// Submission status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmissionStatus {
    Pending,
    Submitted,
    Acknowledged,
    Rejected,
    Failed,
}

/// Report validator
pub struct ReportValidator;

impl ReportValidator {
    /// Validate trade report completeness
    pub fn validate_trade_report(report: &TradeReport) -> Result<()> {
        if report.trade_id.is_empty() {
            anyhow::bail!("Trade ID is required");
        }
        if report.instrument_id.is_empty() {
            anyhow::bail!("Instrument ID is required");
        }
        if report.quantity == Decimal::ZERO {
            anyhow::bail!("Quantity must be non-zero");
        }
        if report.price == Decimal::ZERO {
            anyhow::bail!("Price must be non-zero");
        }
        Ok(())
    }

    /// Validate transaction report
    pub fn validate_transaction_report(report: &TransactionReport) -> Result<()> {
        if report.transaction_id.is_empty() {
            anyhow::bail!("Transaction ID is required");
        }
        if report.account_id.is_empty() {
            anyhow::bail!("Account ID is required");
        }
        if report.amount == Decimal::ZERO {
            anyhow::bail!("Amount must be non-zero");
        }
        Ok(())
    }

    /// Validate position report
    pub fn validate_position_report(report: &PositionReport) -> Result<()> {
        if report.account_id.is_empty() {
            anyhow::bail!("Account ID is required");
        }
        if report.instrument_id.is_empty() {
            anyhow::bail!("Instrument ID is required");
        }
        Ok(())
    }

    /// Validate best execution report
    pub fn validate_best_execution_report(report: &BestExecutionReport) -> Result<()> {
        if report.order_id.is_empty() {
            anyhow::bail!("Order ID is required");
        }
        if report.client_id.is_empty() {
            anyhow::bail!("Client ID is required");
        }
        if report.venues_considered.is_empty() {
            anyhow::bail!("At least one venue must be considered");
        }
        if report.selected_venue.is_empty() {
            anyhow::bail!("Selected venue is required");
        }
        if report.justification.is_empty() {
            anyhow::bail!("Justification for venue selection is required");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_trade_report_generation() {
        let report =
            RegulatoryReportGenerator::generate_trade_report("TRADE123", RegulatoryRegime::MiFIDII);
        assert_eq!(report.trade_id, "TRADE123");
        assert_eq!(report.regime, RegulatoryRegime::MiFIDII);
    }

    #[test]
    fn test_transaction_report_generation() {
        let report = RegulatoryReportGenerator::generate_transaction_report("TXN456");
        assert_eq!(report.transaction_id, "TXN456");
        assert_eq!(report.status, TransactionStatus::Completed);
    }

    #[test]
    fn test_position_report_generation() {
        let report = RegulatoryReportGenerator::generate_position_report("ACC789", "INST001");
        assert_eq!(report.account_id, "ACC789");
        assert_eq!(report.instrument_id, "INST001");
    }

    #[test]
    fn test_best_execution_report_generation() {
        let report = RegulatoryReportGenerator::generate_best_execution_report("ORD001", "CLI001");
        assert_eq!(report.order_id, "ORD001");
        assert_eq!(report.client_id, "CLI001");
    }

    #[test]
    fn test_trade_report_validation_fails_empty_id() {
        let mut report =
            RegulatoryReportGenerator::generate_trade_report("", RegulatoryRegime::EMIR);
        assert!(ReportValidator::validate_trade_report(&report).is_err());

        report.trade_id = "TRADE123".to_string();
        assert!(ReportValidator::validate_trade_report(&report).is_err()); // Still fails due to empty instrument_id
    }

    #[test]
    fn test_trade_report_validation_success() {
        let mut report =
            RegulatoryReportGenerator::generate_trade_report("TRADE123", RegulatoryRegime::EMIR);
        report.instrument_id = "INST001".to_string();
        report.quantity = dec!(100);
        report.price = dec!(50.5);

        assert!(ReportValidator::validate_trade_report(&report).is_ok());
    }

    #[test]
    fn test_transaction_report_validation() {
        let mut report = RegulatoryReportGenerator::generate_transaction_report("TXN123");
        report.account_id = "ACC123".to_string();
        report.amount = dec!(1000);

        assert!(ReportValidator::validate_transaction_report(&report).is_ok());
    }

    #[test]
    fn test_position_report_validation() {
        let report = RegulatoryReportGenerator::generate_position_report("ACC123", "INST001");
        assert!(ReportValidator::validate_position_report(&report).is_ok());
    }

    #[test]
    fn test_best_execution_report_validation() {
        let mut report =
            RegulatoryReportGenerator::generate_best_execution_report("ORD001", "CLI001");
        report.venues_considered = vec!["VENUE1".to_string(), "VENUE2".to_string()];
        report.selected_venue = "VENUE1".to_string();
        report.justification = "Best price and liquidity".to_string();

        assert!(ReportValidator::validate_best_execution_report(&report).is_ok());
    }

    #[test]
    fn test_report_submission_tracking() {
        let submission = ReportSubmission {
            submission_id: "SUB001".to_string(),
            report_type: ReportType::Trade,
            regime: RegulatoryRegime::MiFIDII,
            submission_timestamp: Utc::now(),
            status: SubmissionStatus::Pending,
            acknowledgment_ref: None,
            error_message: None,
        };

        assert_eq!(submission.status, SubmissionStatus::Pending);
        assert_eq!(submission.report_type, ReportType::Trade);
    }
}
