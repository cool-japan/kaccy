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
    NotStarted,
    InProgress,
    PendingReview,
    Approved,
    Rejected,
    Expired,
}

/// Identity document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    Passport,
    DriversLicense,
    NationalId,
    ResidencePermit,
    UtilityBill,
    BankStatement,
}

/// Identity verification request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityVerification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub level: KycLevel,
    pub status: KycStatus,
    pub documents: Vec<VerificationDocument>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
}

/// Verification document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDocument {
    pub id: Uuid,
    pub document_type: DocumentType,
    pub document_number: Option<String>,
    pub issued_country: String,
    pub expiry_date: Option<DateTime<Utc>>,
    pub verified: bool,
}

impl IdentityVerification {
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

    pub fn add_document(&mut self, document: VerificationDocument) {
        self.documents.push(document);
        self.status = KycStatus::InProgress;
        self.updated_at = Utc::now();
    }

    pub fn submit_for_review(&mut self) {
        self.status = KycStatus::PendingReview;
        self.updated_at = Utc::now();
    }

    pub fn approve(&mut self) {
        self.status = KycStatus::Approved;
        self.verified_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn reject(&mut self, reason: String) {
        self.status = KycStatus::Rejected;
        self.rejection_reason = Some(reason);
        self.updated_at = Utc::now();
    }
}

/// AML risk score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    pub user_id: Uuid,
    pub score: u8, // 0-100
    pub risk_level: RiskLevel,
    pub factors: Vec<RiskFactor>,
    pub calculated_at: DateTime<Utc>,
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,      // 0-30
    Medium,   // 31-60
    High,     // 61-80
    Critical, // 81-100
}

impl RiskLevel {
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
    pub factor_type: RiskFactorType,
    pub description: String,
    pub weight: f64,
}

/// Risk factor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskFactorType {
    HighValueTransactions,
    FrequentTransactions,
    NewAccount,
    HighRiskCountry,
    UnusualPattern,
    MultipleAccounts,
    PEP, // Politically Exposed Person
    Sanctions,
}

/// Risk scoring engine
pub struct RiskScoringEngine {
    /// Risk factor weights
    weights: HashMap<RiskFactorType, f64>,
}

impl RiskScoringEngine {
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
    pub id: Uuid,
    pub user_id: Uuid,
    pub trigger_type: EDDTriggerType,
    pub description: String,
    pub triggered_at: DateTime<Utc>,
    pub status: EDDStatus,
}

/// EDD trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EDDTriggerType {
    HighRiskScore,
    LargeTransaction,
    PEPIdentified,
    SanctionsMatch,
    UnusualActivity,
    CustomerRequest,
}

/// EDD status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EDDStatus {
    Pending,
    InProgress,
    Completed,
    Escalated,
}

/// Ongoing monitoring system
pub struct OngoingMonitoring {
    /// Monitored users
    monitored_users: HashMap<Uuid, MonitoringProfile>,
    /// Alerts generated
    alerts: Vec<MonitoringAlert>,
}

/// Monitoring profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringProfile {
    pub user_id: Uuid,
    pub risk_level: RiskLevel,
    pub monitoring_frequency: MonitoringFrequency,
    pub last_review: DateTime<Utc>,
    pub next_review: DateTime<Utc>,
}

/// Monitoring frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitoringFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
}

/// Monitoring alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringAlert {
    pub id: Uuid,
    pub user_id: Uuid,
    pub alert_type: MonitoringAlertType,
    pub severity: AlertSeverity,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
}

/// Monitoring alert type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitoringAlertType {
    TransactionPatternChange,
    VolumeIncrease,
    NewRiskFactor,
    DocumentExpiring,
    RiskScoreIncrease,
}

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl OngoingMonitoring {
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
