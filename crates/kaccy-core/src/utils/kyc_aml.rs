//! KYC (Know Your Customer) and AML (Anti-Money Laundering) Integration
//!
//! This module provides comprehensive KYC/AML capabilities including:
//! - Identity verification workflows
//! - Risk scoring automation
//! - Enhanced due diligence triggers
//! - Ongoing monitoring

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// KYC verification level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KycLevel {
    /// No verification
    None,
    /// Basic verification (email, phone)
    Basic,
    /// Intermediate (ID document)
    Intermediate,
    /// Advanced (ID + address proof)
    Advanced,
    /// Full (video verification)
    Full,
}

/// KYC status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KycStatus {
    /// Verification not yet started
    NotStarted,
    /// Documents submitted, verification in progress
    InProgress,
    /// Submitted and awaiting manual review
    PendingReview,
    /// Verification approved
    Approved,
    /// Verification rejected
    Rejected,
    /// Verification has expired
    Expired,
}

/// Identity document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    /// International travel passport
    Passport,
    /// Government-issued driver's licence
    DriversLicense,
    /// National identity card
    NationalId,
    /// Residence permit
    ResidencePermit,
    /// Utility bill for address verification
    UtilityBill,
    /// Bank statement for address verification
    BankStatement,
}

/// Identity verification request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityVerification {
    /// Unique identifier of this verification request
    pub id: Uuid,
    /// User being verified
    pub user_id: Uuid,
    /// Requested KYC level
    pub level: KycLevel,
    /// Current status of the verification
    pub status: KycStatus,
    /// Documents attached to this verification
    pub documents: Vec<VerificationDocument>,
    /// Timestamp when the verification was created
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent status change
    pub updated_at: DateTime<Utc>,
    /// Timestamp when verification was approved
    pub verified_at: Option<DateTime<Utc>>,
    /// Reason if verification was rejected
    pub rejection_reason: Option<String>,
}

/// Verification document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDocument {
    /// Unique identifier of this document
    pub id: Uuid,
    /// Type of the document
    pub document_type: DocumentType,
    /// Document reference number (e.g. passport number)
    pub document_number: Option<String>,
    /// Country that issued the document
    pub issued_country: String,
    /// Expiry date of the document
    pub expiry_date: Option<DateTime<Utc>>,
    /// Whether this document has been verified
    pub verified: bool,
}

impl IdentityVerification {
    /// Create a new identity verification request for the given user and level
    pub fn new(user_id: Uuid, level: KycLevel) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            level,
            status: KycStatus::NotStarted,
            documents: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            verified_at: None,
            rejection_reason: None,
        }
    }

    /// Attach a verification document to this request
    pub fn add_document(&mut self, document: VerificationDocument) {
        self.documents.push(document);
        self.status = KycStatus::InProgress;
        self.updated_at = Utc::now();
    }

    /// Submit the completed verification for review
    pub fn submit_for_review(&mut self) {
        self.status = KycStatus::PendingReview;
        self.updated_at = Utc::now();
    }

    /// Mark the verification as approved
    pub fn approve(&mut self) {
        self.status = KycStatus::Approved;
        self.verified_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Reject the verification with a reason
    pub fn reject(&mut self, reason: String) {
        self.status = KycStatus::Rejected;
        self.rejection_reason = Some(reason);
        self.updated_at = Utc::now();
    }
}

/// AML risk score for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    /// User being scored
    pub user_id: Uuid,
    /// Numeric score from 0 (no risk) to 100 (maximum risk)
    pub score: u8,
    /// Derived risk level category
    pub risk_level: RiskLevel,
    /// Individual risk factors that contributed to this score
    pub factors: Vec<RiskFactor>,
    /// Timestamp when this score was calculated
    pub calculated_at: DateTime<Utc>,
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk — score 0–30
    Low,
    /// Medium risk — score 31–60
    Medium,
    /// High risk — score 61–80
    High,
    /// Critical risk — score 81–100
    Critical,
}

impl RiskLevel {
    /// Derive the risk level classification from a numeric score (0–100)
    pub fn from_score(score: u8) -> Self {
        match score {
            0..=30 => RiskLevel::Low,
            31..=60 => RiskLevel::Medium,
            61..=80 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }
}

/// Risk factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// Category of this risk factor
    pub factor_type: RiskFactorType,
    /// Human-readable description
    pub description: String,
    /// Contribution of this factor to the overall risk score
    pub weight: f64,
}

/// Risk factor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskFactorType {
    /// Transaction values exceeded reporting threshold
    HighValueTransactions,
    /// Unusually high number of transactions in a period
    FrequentTransactions,
    /// Account created recently
    NewAccount,
    /// User is based in a high-risk jurisdiction
    HighRiskCountry,
    /// Detected unusual behavioural pattern
    UnusualPattern,
    /// User operates multiple linked accounts
    MultipleAccounts,
    /// Politically Exposed Person classification
    PEP,
    /// User appears on a sanctions list
    Sanctions,
}

/// Risk scoring engine
pub struct RiskScoringEngine {
    /// Risk factor weights
    weights: HashMap<RiskFactorType, f64>,
}

impl RiskScoringEngine {
    /// Create a new risk scoring engine with default factor weights
    pub fn new() -> Self {
        let mut weights = HashMap::new();
        weights.insert(RiskFactorType::HighValueTransactions, 15.0);
        weights.insert(RiskFactorType::FrequentTransactions, 10.0);
        weights.insert(RiskFactorType::NewAccount, 5.0);
        weights.insert(RiskFactorType::HighRiskCountry, 20.0);
        weights.insert(RiskFactorType::UnusualPattern, 15.0);
        weights.insert(RiskFactorType::MultipleAccounts, 10.0);
        weights.insert(RiskFactorType::PEP, 25.0);
        weights.insert(RiskFactorType::Sanctions, 50.0);

        Self { weights }
    }

    /// Calculate risk score
    pub fn calculate_score(&self, user_id: Uuid, factors: Vec<RiskFactor>) -> RiskScore {
        let total_score: f64 = factors.iter().map(|f| f.weight).sum();

        let score = total_score.min(100.0) as u8;
        let risk_level = RiskLevel::from_score(score);

        RiskScore {
            user_id,
            score,
            risk_level,
            factors,
            calculated_at: Utc::now(),
        }
    }

    /// Auto-calculate risk based on user activity
    #[allow(clippy::too_many_arguments)]
    pub fn auto_calculate(
        &self,
        user_id: Uuid,
        account_age_days: u32,
        transaction_count: usize,
        total_volume: Decimal,
        high_risk_country: bool,
        is_pep: bool,
        is_sanctioned: bool,
    ) -> RiskScore {
        let mut factors = Vec::new();

        // New account risk
        if account_age_days < 30 {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::NewAccount,
                description: "Account created less than 30 days ago".to_string(),
                weight: *self
                    .weights
                    .get(&RiskFactorType::NewAccount)
                    .unwrap_or(&5.0),
            });
        }

        // High value transactions
        if total_volume > Decimal::new(10000, 0) {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::HighValueTransactions,
                description: format!("Total volume: {}", total_volume),
                weight: *self
                    .weights
                    .get(&RiskFactorType::HighValueTransactions)
                    .unwrap_or(&15.0),
            });
        }

        // Frequent transactions
        if transaction_count > 100 {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::FrequentTransactions,
                description: format!("{} transactions detected", transaction_count),
                weight: *self
                    .weights
                    .get(&RiskFactorType::FrequentTransactions)
                    .unwrap_or(&10.0),
            });
        }

        // High-risk country
        if high_risk_country {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::HighRiskCountry,
                description: "User from high-risk jurisdiction".to_string(),
                weight: *self
                    .weights
                    .get(&RiskFactorType::HighRiskCountry)
                    .unwrap_or(&20.0),
            });
        }

        // PEP status
        if is_pep {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::PEP,
                description: "Politically Exposed Person".to_string(),
                weight: *self.weights.get(&RiskFactorType::PEP).unwrap_or(&25.0),
            });
        }

        // Sanctions
        if is_sanctioned {
            factors.push(RiskFactor {
                factor_type: RiskFactorType::Sanctions,
                description: "User on sanctions list".to_string(),
                weight: *self
                    .weights
                    .get(&RiskFactorType::Sanctions)
                    .unwrap_or(&50.0),
            });
        }

        self.calculate_score(user_id, factors)
    }
}

impl Default for RiskScoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced Due Diligence (EDD) trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EDDTrigger {
    /// Unique identifier of this trigger
    pub id: Uuid,
    /// User for whom EDD was triggered
    pub user_id: Uuid,
    /// What caused the EDD to be triggered
    pub trigger_type: EDDTriggerType,
    /// Human-readable description of the trigger
    pub description: String,
    /// Timestamp when the trigger fired
    pub triggered_at: DateTime<Utc>,
    /// Current status of this EDD case
    pub status: EDDStatus,
}

/// EDD trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EDDTriggerType {
    /// Risk score crossed the EDD threshold
    HighRiskScore,
    /// Transaction value exceeded the large-transaction threshold
    LargeTransaction,
    /// User identified as a Politically Exposed Person
    PEPIdentified,
    /// Sanctions list match detected
    SanctionsMatch,
    /// Unusual activity pattern detected
    UnusualActivity,
    /// Customer requested enhanced due diligence
    CustomerRequest,
}

/// EDD status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EDDStatus {
    /// EDD case is open and awaiting review
    Pending,
    /// EDD review is currently in progress
    InProgress,
    /// EDD review has been completed
    Completed,
    /// EDD case has been escalated to compliance
    Escalated,
}

/// Ongoing monitoring system
pub struct OngoingMonitoring {
    /// Monitored users indexed by user ID
    monitored_users: HashMap<Uuid, MonitoringProfile>,
    /// Alerts generated by the monitoring system
    alerts: Vec<MonitoringAlert>,
}

/// Monitoring profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringProfile {
    /// User being monitored
    pub user_id: Uuid,
    /// Current risk classification
    pub risk_level: RiskLevel,
    /// How often this user is reviewed
    pub monitoring_frequency: MonitoringFrequency,
    /// Timestamp of the last completed review
    pub last_review: DateTime<Utc>,
    /// Scheduled time for the next review
    pub next_review: DateTime<Utc>,
}

/// Monitoring frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitoringFrequency {
    /// Reviewed every day
    Daily,
    /// Reviewed every week
    Weekly,
    /// Reviewed every month
    Monthly,
    /// Reviewed every quarter
    Quarterly,
}

/// Monitoring alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringAlert {
    /// Unique identifier of this alert
    pub id: Uuid,
    /// User this alert concerns
    pub user_id: Uuid,
    /// What kind of alert was raised
    pub alert_type: MonitoringAlertType,
    /// How serious this alert is
    pub severity: AlertSeverity,
    /// Human-readable description of the alert
    pub description: String,
    /// When this alert was generated
    pub created_at: DateTime<Utc>,
    /// Whether this alert has been resolved
    pub resolved: bool,
}

/// Monitoring alert type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitoringAlertType {
    /// Observed transaction behaviour has changed significantly
    TransactionPatternChange,
    /// Transaction volume has increased beyond expected levels
    VolumeIncrease,
    /// A new risk factor was identified
    NewRiskFactor,
    /// A verification document is about to expire
    DocumentExpiring,
    /// Risk score has increased above a threshold
    RiskScoreIncrease,
}

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational — no immediate action required
    Info,
    /// Warrants attention and possible action
    Warning,
    /// Requires immediate action
    Critical,
}

impl OngoingMonitoring {
    /// Create a new ongoing monitoring system
    pub fn new() -> Self {
        Self {
            monitored_users: HashMap::new(),
            alerts: Vec::new(),
        }
    }

    /// Add user to monitoring
    pub fn add_user(&mut self, user_id: Uuid, risk_level: RiskLevel) {
        let frequency = match risk_level {
            RiskLevel::Low => MonitoringFrequency::Quarterly,
            RiskLevel::Medium => MonitoringFrequency::Monthly,
            RiskLevel::High => MonitoringFrequency::Weekly,
            RiskLevel::Critical => MonitoringFrequency::Daily,
        };

        let now = Utc::now();
        let next_review = match frequency {
            MonitoringFrequency::Daily => now + chrono::Duration::days(1),
            MonitoringFrequency::Weekly => now + chrono::Duration::weeks(1),
            MonitoringFrequency::Monthly => now + chrono::Duration::days(30),
            MonitoringFrequency::Quarterly => now + chrono::Duration::days(90),
        };

        self.monitored_users.insert(
            user_id,
            MonitoringProfile {
                user_id,
                risk_level,
                monitoring_frequency: frequency,
                last_review: now,
                next_review,
            },
        );
    }

    /// Create alert
    pub fn create_alert(
        &mut self,
        user_id: Uuid,
        alert_type: MonitoringAlertType,
        severity: AlertSeverity,
        description: String,
    ) {
        self.alerts.push(MonitoringAlert {
            id: Uuid::new_v4(),
            user_id,
            alert_type,
            severity,
            description,
            created_at: Utc::now(),
            resolved: false,
        });
    }

    /// Get unresolved alerts
    pub fn get_unresolved_alerts(&self) -> Vec<&MonitoringAlert> {
        self.alerts.iter().filter(|a| !a.resolved).collect()
    }

    /// Get users due for review
    pub fn get_users_due_for_review(&self) -> Vec<&MonitoringProfile> {
        let now = Utc::now();
        self.monitored_users
            .values()
            .filter(|p| p.next_review <= now)
            .collect()
    }
}

impl Default for OngoingMonitoring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_identity_verification_workflow() {
        let user_id = Uuid::new_v4();
        let mut verification = IdentityVerification::new(user_id, KycLevel::Advanced);

        assert_eq!(verification.status, KycStatus::NotStarted);

        let document = VerificationDocument {
            id: Uuid::new_v4(),
            document_type: DocumentType::Passport,
            document_number: Some("ABC123".to_string()),
            issued_country: "US".to_string(),
            expiry_date: Some(Utc::now() + chrono::Duration::days(365)),
            verified: false,
        };

        verification.add_document(document);
        assert_eq!(verification.status, KycStatus::InProgress);

        verification.submit_for_review();
        assert_eq!(verification.status, KycStatus::PendingReview);

        verification.approve();
        assert_eq!(verification.status, KycStatus::Approved);
        assert!(verification.verified_at.is_some());
    }

    #[test]
    fn test_identity_verification_rejection() {
        let user_id = Uuid::new_v4();
        let mut verification = IdentityVerification::new(user_id, KycLevel::Basic);

        verification.reject("Document not clear".to_string());
        assert_eq!(verification.status, KycStatus::Rejected);
        assert!(verification.rejection_reason.is_some());
    }

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(25), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(50), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(70), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(90), RiskLevel::Critical);
    }

    #[test]
    fn test_risk_scoring_engine() {
        let engine = RiskScoringEngine::new();
        let user_id = Uuid::new_v4();

        let risk_score = engine.auto_calculate(
            user_id,
            10,          // 10 days old account
            150,         // 150 transactions
            dec!(50000), // $50k volume
            true,        // high-risk country
            false,       // not PEP
            false,       // not sanctioned
        );

        assert!(risk_score.score > 0);
        assert_eq!(
            risk_score.risk_level,
            RiskLevel::from_score(risk_score.score)
        );
        assert!(!risk_score.factors.is_empty());
    }

    #[test]
    fn test_high_risk_scoring() {
        let engine = RiskScoringEngine::new();
        let user_id = Uuid::new_v4();

        let risk_score = engine.auto_calculate(
            user_id,
            5,            // 5 days old
            200,          // many transactions
            dec!(100000), // high volume
            true,         // high-risk country
            true,         // is PEP
            false,        // not sanctioned
        );

        assert!(risk_score.score >= 60); // Should be high risk
        assert!(matches!(
            risk_score.risk_level,
            RiskLevel::High | RiskLevel::Critical
        ));
    }

    #[test]
    fn test_sanctions_risk() {
        let engine = RiskScoringEngine::new();
        let user_id = Uuid::new_v4();

        let risk_score = engine.auto_calculate(
            user_id,
            100,       // old account
            10,        // few transactions
            dec!(100), // low volume
            false,     // not high-risk country
            false,     // not PEP
            true,      // IS sanctioned
        );

        // Sanctions should result in at least Medium risk or higher
        assert!(matches!(
            risk_score.risk_level,
            RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical
        ));
        assert!(risk_score.score >= 50); // Sanctions alone is 50
    }

    #[test]
    fn test_ongoing_monitoring() {
        let mut monitoring = OngoingMonitoring::new();
        let user_id = Uuid::new_v4();

        monitoring.add_user(user_id, RiskLevel::High);

        assert!(monitoring.monitored_users.contains_key(&user_id));

        let profile = monitoring.monitored_users.get(&user_id).unwrap();
        assert_eq!(profile.monitoring_frequency, MonitoringFrequency::Weekly);
    }

    #[test]
    fn test_monitoring_alerts() {
        let mut monitoring = OngoingMonitoring::new();
        let user_id = Uuid::new_v4();

        monitoring.create_alert(
            user_id,
            MonitoringAlertType::VolumeIncrease,
            AlertSeverity::Warning,
            "Volume increased by 300%".to_string(),
        );

        let unresolved = monitoring.get_unresolved_alerts();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].severity, AlertSeverity::Warning);
    }

    #[test]
    fn test_monitoring_frequency_by_risk() {
        let mut monitoring = OngoingMonitoring::new();

        monitoring.add_user(Uuid::new_v4(), RiskLevel::Low);
        monitoring.add_user(Uuid::new_v4(), RiskLevel::Critical);

        let profiles: Vec<&MonitoringProfile> = monitoring.monitored_users.values().collect();

        let low_risk = profiles
            .iter()
            .find(|p| p.risk_level == RiskLevel::Low)
            .unwrap();
        assert_eq!(
            low_risk.monitoring_frequency,
            MonitoringFrequency::Quarterly
        );

        let critical_risk = profiles
            .iter()
            .find(|p| p.risk_level == RiskLevel::Critical)
            .unwrap();
        assert_eq!(
            critical_risk.monitoring_frequency,
            MonitoringFrequency::Daily
        );
    }
}
