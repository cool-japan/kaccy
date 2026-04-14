//! Compliance and regulatory framework for transaction monitoring

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Geographic region for compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    /// United States
    US,
    /// European Union
    EU,
    /// United Kingdom
    UK,
    /// Asia Pacific
    APAC,
    /// Middle East
    ME,
    /// Latin America
    LATAM,
    /// Other/Unknown
    Other,
}

/// Trading restriction level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestrictionLevel {
    /// No restrictions
    None,
    /// Partial restrictions (limits apply)
    Partial,
    /// Fully restricted
    Full,
}

/// Suspicious activity type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuspiciousActivity {
    /// Unusual trading volume
    UnusualVolume,
    /// Rapid account funding
    RapidFunding,
    /// Wash trading pattern
    WashTrading,
    /// Layering pattern
    Layering,
    /// Spoofing pattern
    Spoofing,
    /// Potential front-running
    FrontRunning,
    /// Unusual withdrawal pattern
    UnusualWithdrawal,
    /// Multiple failed transactions
    FailedTransactions,
}

/// Suspicious activity alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousActivityAlert {
    /// Alert ID
    pub id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Activity type
    pub activity_type: SuspiciousActivity,
    /// Description
    pub description: String,
    /// Risk score (0-100)
    pub risk_score: u8,
    /// Related transactions
    pub transaction_ids: Vec<Uuid>,
    /// Detected at
    pub detected_at: DateTime<Utc>,
    /// Reviewed
    pub reviewed: bool,
    /// Reviewer notes
    pub notes: Option<String>,
}

/// Regional compliance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalCompliance {
    /// Region
    pub region: Region,
    /// Whether trading is allowed
    pub trading_allowed: bool,
    /// Daily trading limit
    pub daily_limit: Option<Decimal>,
    /// Monthly trading limit
    pub monthly_limit: Option<Decimal>,
    /// KYC required
    pub kyc_required: bool,
    /// Minimum age requirement
    pub min_age: u8,
    /// Restricted tokens
    pub restricted_tokens: HashSet<Uuid>,
}

impl RegionalCompliance {
    /// Create standard US compliance
    pub fn us_standard() -> Self {
        Self {
            region: Region::US,
            trading_allowed: true,
            daily_limit: Some(dec!(50000)),
            monthly_limit: Some(dec!(500000)),
            kyc_required: true,
            min_age: 18,
            restricted_tokens: HashSet::new(),
        }
    }

    /// Create standard EU compliance
    pub fn eu_standard() -> Self {
        Self {
            region: Region::EU,
            trading_allowed: true,
            daily_limit: Some(dec!(40000)),
            monthly_limit: Some(dec!(400000)),
            kyc_required: true,
            min_age: 18,
            restricted_tokens: HashSet::new(),
        }
    }

    /// Check if token is restricted
    pub fn is_token_restricted(&self, token_id: Uuid) -> bool {
        self.restricted_tokens.contains(&token_id)
    }

    /// Check if amount exceeds limits
    pub fn exceeds_limits(&self, daily_volume: Decimal, monthly_volume: Decimal) -> bool {
        if let Some(daily_limit) = self.daily_limit {
            if daily_volume > daily_limit {
                return true;
            }
        }

        if let Some(monthly_limit) = self.monthly_limit {
            if monthly_volume > monthly_limit {
                return true;
            }
        }

        false
    }
}

/// Transaction monitor for compliance
pub struct TransactionMonitor {
    /// Accumulated daily trading volume per user
    daily_volumes: HashMap<Uuid, Decimal>,
    /// Accumulated monthly trading volume per user
    monthly_volumes: HashMap<Uuid, Decimal>,
    /// Generated suspicious activity alerts
    alerts: Vec<SuspiciousActivityAlert>,
    /// Compliance rules keyed by region
    regional_rules: HashMap<Region, RegionalCompliance>,
    /// Assigned region for each user
    user_regions: HashMap<Uuid, Region>,
}

impl TransactionMonitor {
    /// Create a new transaction monitor
    pub fn new() -> Self {
        let mut regional_rules = HashMap::new();
        regional_rules.insert(Region::US, RegionalCompliance::us_standard());
        regional_rules.insert(Region::EU, RegionalCompliance::eu_standard());

        Self {
            daily_volumes: HashMap::new(),
            monthly_volumes: HashMap::new(),
            alerts: Vec::new(),
            regional_rules,
            user_regions: HashMap::new(),
        }
    }

    /// Set user region
    pub fn set_user_region(&mut self, user_id: Uuid, region: Region) {
        self.user_regions.insert(user_id, region);
    }

    /// Add regional rule
    pub fn add_regional_rule(&mut self, compliance: RegionalCompliance) {
        self.regional_rules.insert(compliance.region, compliance);
    }

    /// Check if transaction is allowed
    pub fn check_transaction(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        amount: Decimal,
    ) -> Result<(), String> {
        let region = self
            .user_regions
            .get(&user_id)
            .copied()
            .unwrap_or(Region::Other);

        let compliance = self
            .regional_rules
            .get(&region)
            .ok_or_else(|| format!("No compliance rules for region {:?}", region))?;

        if !compliance.trading_allowed {
            return Err("Trading not allowed in your region".to_string());
        }

        if compliance.is_token_restricted(token_id) {
            return Err("Token is restricted in your region".to_string());
        }

        let daily_volume = self
            .daily_volumes
            .get(&user_id)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let monthly_volume = self
            .monthly_volumes
            .get(&user_id)
            .copied()
            .unwrap_or(Decimal::ZERO);

        if compliance.exceeds_limits(daily_volume + amount, monthly_volume + amount) {
            return Err("Transaction would exceed regional limits".to_string());
        }

        Ok(())
    }

    /// Record a transaction
    pub fn record_transaction(&mut self, user_id: Uuid, amount: Decimal) {
        *self.daily_volumes.entry(user_id).or_insert(Decimal::ZERO) += amount;
        *self.monthly_volumes.entry(user_id).or_insert(Decimal::ZERO) += amount;
    }

    /// Detect suspicious activity
    pub fn detect_suspicious_activity(
        &mut self,
        user_id: Uuid,
        transaction_ids: Vec<Uuid>,
        description: String,
        activity_type: SuspiciousActivity,
        risk_score: u8,
    ) {
        let alert = SuspiciousActivityAlert {
            id: Uuid::new_v4(),
            user_id,
            activity_type,
            description,
            risk_score,
            transaction_ids,
            detected_at: Utc::now(),
            reviewed: false,
            notes: None,
        };

        self.alerts.push(alert);
    }

    /// Get unreviewed alerts
    pub fn get_unreviewed_alerts(&self) -> Vec<&SuspiciousActivityAlert> {
        self.alerts.iter().filter(|a| !a.reviewed).collect()
    }

    /// Get high-risk alerts
    pub fn get_high_risk_alerts(&self, threshold: u8) -> Vec<&SuspiciousActivityAlert> {
        self.alerts
            .iter()
            .filter(|a| a.risk_score >= threshold)
            .collect()
    }

    /// Review an alert
    pub fn review_alert(&mut self, alert_id: Uuid, notes: String) -> Result<(), String> {
        if let Some(alert) = self.alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.reviewed = true;
            alert.notes = Some(notes);
            Ok(())
        } else {
            Err("Alert not found".to_string())
        }
    }

    /// Reset daily volumes (should be called daily)
    pub fn reset_daily_volumes(&mut self) {
        self.daily_volumes.clear();
    }

    /// Reset monthly volumes (should be called monthly)
    pub fn reset_monthly_volumes(&mut self) {
        self.monthly_volumes.clear();
    }

    /// Generate compliance report
    pub fn generate_report(&self) -> ComplianceReport {
        let total_users = self.user_regions.len();
        let total_alerts = self.alerts.len();
        let unreviewed_alerts = self.get_unreviewed_alerts().len();
        let high_risk_alerts = self.get_high_risk_alerts(75).len();

        let mut users_by_region: HashMap<Region, usize> = HashMap::new();
        for region in self.user_regions.values() {
            *users_by_region.entry(*region).or_insert(0) += 1;
        }

        ComplianceReport {
            generated_at: Utc::now(),
            total_users,
            total_alerts,
            unreviewed_alerts,
            high_risk_alerts,
            users_by_region,
        }
    }
}

impl Default for TransactionMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Report generation time
    pub generated_at: DateTime<Utc>,
    /// Total users
    pub total_users: usize,
    /// Total alerts
    pub total_alerts: usize,
    /// Unreviewed alerts
    pub unreviewed_alerts: usize,
    /// High-risk alerts
    pub high_risk_alerts: usize,
    /// Users by region
    pub users_by_region: HashMap<Region, usize>,
}

/// Real-time trade surveillance system
pub struct RealTimeSurveillance {
    /// Active surveillance rules to evaluate
    rules: Vec<SurveillanceRule>,
    /// Violations detected so far
    violations: Vec<Violation>,
}

/// Surveillance rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurveillanceRule {
    /// Unique identifier of the rule
    pub id: Uuid,
    /// Human-readable name of the rule
    pub name: String,
    /// Category of market behaviour this rule detects
    pub rule_type: RuleType,
    /// Whether this rule is currently active
    pub enabled: bool,
    /// Optional numeric threshold that triggers the rule
    pub threshold: Option<f64>,
}

/// Rule type for surveillance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    /// Abnormal spike in trading volume
    VolumeSpike,
    /// Suspected price manipulation
    PriceManipulation,
    /// Suspected insider trading
    InsiderTrading,
    /// General market manipulation
    MarketManipulation,
    /// Order layering to deceive the market
    Layering,
    /// Spoofing of the order book
    Spoofing,
}

/// Violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// Unique identifier of this violation
    pub id: Uuid,
    /// ID of the rule that was triggered
    pub rule_id: Uuid,
    /// User responsible for the violation
    pub user_id: Uuid,
    /// When the violation was detected
    pub detected_at: DateTime<Utc>,
    /// How severe the violation is
    pub severity: ViolationSeverity,
    /// Human-readable description of the violation
    pub description: String,
    /// Supporting evidence items
    pub evidence: Vec<String>,
}

/// Violation severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    /// Minor infraction
    Low,
    /// Moderate infraction
    Medium,
    /// Serious infraction
    High,
    /// Severe infraction requiring immediate action
    Critical,
}

impl RealTimeSurveillance {
    /// Create a new real-time surveillance system
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// Register a new surveillance rule
    pub fn add_rule(&mut self, rule: SurveillanceRule) {
        self.rules.push(rule);
    }

    /// Record a detected violation
    pub fn record_violation(&mut self, violation: Violation) {
        self.violations.push(violation);
    }

    /// Return all violations matching the given severity
    pub fn get_violations_by_severity(&self, severity: ViolationSeverity) -> Vec<&Violation> {
        self.violations
            .iter()
            .filter(|v| v.severity == severity)
            .collect()
    }
}

impl Default for RealTimeSurveillance {
    fn default() -> Self {
        Self::new()
    }
}

/// Automated Suspicious Activity Report (SAR) generator
pub struct AutomatedSAR {
    /// SARs awaiting submission
    pending_sars: Vec<SuspiciousActivityReport>,
    /// SARs already submitted to regulators
    submitted_sars: Vec<SuspiciousActivityReport>,
}

/// Suspicious Activity Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousActivityReport {
    /// Unique identifier of this SAR
    pub id: Uuid,
    /// Subject user
    pub user_id: Uuid,
    /// Nature of the suspicious activity
    pub activity_type: SuspiciousActivity,
    /// Narrative description of the suspicious activity
    pub description: String,
    /// Start of the observation period
    pub start_date: DateTime<Utc>,
    /// End of the observation period
    pub end_date: DateTime<Utc>,
    /// Aggregate value of transactions in this SAR
    pub total_amount: Decimal,
    /// Number of transactions covered
    pub transaction_count: usize,
    /// Specific risk indicators observed
    pub risk_indicators: Vec<String>,
    /// Current lifecycle status of the SAR
    pub status: SARStatus,
    /// When the SAR was drafted
    pub created_at: DateTime<Utc>,
    /// When the SAR was submitted to regulators
    pub submitted_at: Option<DateTime<Utc>>,
}

/// SAR status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SARStatus {
    /// SAR is being drafted
    Draft,
    /// SAR is awaiting compliance officer review
    PendingReview,
    /// SAR has been approved for submission
    Approved,
    /// SAR has been submitted to regulators
    Submitted,
    /// SAR was rejected and will not be filed
    Rejected,
}

impl AutomatedSAR {
    /// Create a new SAR generator
    pub fn new() -> Self {
        Self {
            pending_sars: Vec::new(),
            submitted_sars: Vec::new(),
        }
    }

    /// Create a new SAR
    #[allow(clippy::too_many_arguments)]
    pub fn create_sar(
        &mut self,
        user_id: Uuid,
        activity_type: SuspiciousActivity,
        description: String,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        total_amount: Decimal,
        transaction_count: usize,
    ) -> Uuid {
        let sar = SuspiciousActivityReport {
            id: Uuid::new_v4(),
            user_id,
            activity_type,
            description,
            start_date,
            end_date,
            total_amount,
            transaction_count,
            risk_indicators: Vec::new(),
            status: SARStatus::Draft,
            created_at: Utc::now(),
            submitted_at: None,
        };

        let id = sar.id;
        self.pending_sars.push(sar);
        id
    }

    /// Submit SAR
    pub fn submit_sar(&mut self, sar_id: Uuid) -> Result<(), String> {
        if let Some(pos) = self.pending_sars.iter().position(|s| s.id == sar_id) {
            let mut sar = self.pending_sars.remove(pos);
            sar.status = SARStatus::Submitted;
            sar.submitted_at = Some(Utc::now());
            self.submitted_sars.push(sar);
            Ok(())
        } else {
            Err("SAR not found".to_string())
        }
    }

    /// Get pending SARs
    pub fn get_pending_sars(&self) -> &[SuspiciousActivityReport] {
        &self.pending_sars
    }
}

impl Default for AutomatedSAR {
    fn default() -> Self {
        Self::new()
    }
}

/// Market manipulation detector
pub struct MarketManipulationDetector {
    /// Relative price-change threshold that triggers an alert
    threshold: f64,
    /// Ordered historical price observations (timestamp, price)
    price_history: Vec<(DateTime<Utc>, Decimal)>,
}

impl MarketManipulationDetector {
    /// Create a new detector with the given price-change threshold
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            price_history: Vec::new(),
        }
    }

    /// Add price point
    pub fn add_price_point(&mut self, timestamp: DateTime<Utc>, price: Decimal) {
        self.price_history.push((timestamp, price));
    }

    /// Detect pump and dump
    pub fn detect_pump_and_dump(&self, window_size: usize) -> Option<ManipulationAlert> {
        if self.price_history.len() < window_size {
            return None;
        }

        let recent = &self.price_history[self.price_history.len() - window_size..];
        let prices: Vec<f64> = recent
            .iter()
            .map(|(_, p)| p.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let max_price = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let price_range = (max_price - min_price) / min_price;

        if price_range > self.threshold {
            Some(ManipulationAlert {
                alert_type: ManipulationType::PumpAndDump,
                detected_at: Utc::now(),
                confidence: (price_range / self.threshold).min(1.0),
                description: format!(
                    "Price increased by {:.2}% then dropped, indicating potential pump and dump",
                    price_range * 100.0
                ),
            })
        } else {
            None
        }
    }

    /// Detect wash trading
    pub fn detect_wash_trading(
        &self,
        user_trades: &[(Uuid, Uuid, Decimal)], // (buyer_id, seller_id, amount)
    ) -> Option<ManipulationAlert> {
        let mut same_party_trades = 0;
        for (buyer, seller, _) in user_trades {
            if buyer == seller {
                same_party_trades += 1;
            }
        }

        let wash_trade_ratio = same_party_trades as f64 / user_trades.len() as f64;

        if wash_trade_ratio > 0.3 {
            // 30% threshold
            Some(ManipulationAlert {
                alert_type: ManipulationType::WashTrading,
                detected_at: Utc::now(),
                confidence: wash_trade_ratio,
                description: format!(
                    "{:.1}% of trades are between same parties",
                    wash_trade_ratio * 100.0
                ),
            })
        } else {
            None
        }
    }
}

/// Manipulation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManipulationType {
    /// Pump-and-dump price inflation scheme
    PumpAndDump,
    /// Wash trading between the same parties
    WashTrading,
    /// Layering of fake orders to move price
    Layering,
    /// Spoofing the order book with cancelled orders
    Spoofing,
}

/// Manipulation alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManipulationAlert {
    /// Category of manipulation detected
    pub alert_type: ManipulationType,
    /// When the manipulation was detected
    pub detected_at: DateTime<Utc>,
    /// Confidence score in the range [0.0, 1.0]
    pub confidence: f64,
    /// Human-readable description of the detection
    pub description: String,
}

/// Insider trading detector
pub struct InsiderTradingDetector {
    /// Trades that occurred suspiciously close to significant events
    suspicious_trades: Vec<SuspiciousTrade>,
}

/// Suspicious trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousTrade {
    /// Identifier of the trade being scrutinised
    pub trade_id: Uuid,
    /// User who placed the trade
    pub user_id: Uuid,
    /// Token that was traded
    pub token_id: Uuid,
    /// Notional amount of the trade
    pub amount: Decimal,
    /// When the trade was executed
    pub trade_timestamp: DateTime<Utc>,
    /// When the related event occurred
    pub event_timestamp: Option<DateTime<Utc>>,
    /// Category of the related event
    pub event_type: Option<String>,
    /// Computed suspicion score in [0.0, 1.0]
    pub suspicion_score: f64,
}

impl InsiderTradingDetector {
    /// Create a new insider trading detector
    pub fn new() -> Self {
        Self {
            suspicious_trades: Vec::new(),
        }
    }

    /// Detect trades before price-moving events
    #[allow(clippy::too_many_arguments)]
    pub fn detect_pre_event_trading(
        &mut self,
        trade_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
        amount: Decimal,
        trade_timestamp: DateTime<Utc>,
        event_timestamp: DateTime<Utc>,
        event_type: String,
    ) {
        let time_diff = (event_timestamp - trade_timestamp).num_hours();

        // Suspicious if trade was made 1-72 hours before event
        if time_diff > 0 && time_diff <= 72 {
            let suspicion_score = 1.0 - (time_diff as f64 / 72.0);

            self.suspicious_trades.push(SuspiciousTrade {
                trade_id,
                user_id,
                token_id,
                amount,
                trade_timestamp,
                event_timestamp: Some(event_timestamp),
                event_type: Some(event_type),
                suspicion_score,
            });
        }
    }

    /// Get high-confidence suspicious trades
    pub fn get_high_confidence_trades(&self, threshold: f64) -> Vec<&SuspiciousTrade> {
        self.suspicious_trades
            .iter()
            .filter(|t| t.suspicion_score >= threshold)
            .collect()
    }
}

impl Default for InsiderTradingDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regional_compliance() {
        let us_compliance = RegionalCompliance::us_standard();

        assert!(us_compliance.trading_allowed);
        assert!(us_compliance.kyc_required);
        assert_eq!(us_compliance.min_age, 18);
    }

    #[test]
    fn test_token_restriction() {
        let mut compliance = RegionalCompliance::us_standard();
        let token_id = Uuid::new_v4();

        assert!(!compliance.is_token_restricted(token_id));

        compliance.restricted_tokens.insert(token_id);
        assert!(compliance.is_token_restricted(token_id));
    }

    #[test]
    fn test_exceeds_limits() {
        let compliance = RegionalCompliance::us_standard();

        assert!(!compliance.exceeds_limits(dec!(1000), dec!(10000)));
        assert!(compliance.exceeds_limits(dec!(60000), dec!(10000))); // Exceeds daily
        assert!(compliance.exceeds_limits(dec!(1000), dec!(600000))); // Exceeds monthly
    }

    #[test]
    fn test_transaction_monitoring() {
        let mut monitor = TransactionMonitor::new();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        monitor.set_user_region(user_id, Region::US);

        let result = monitor.check_transaction(user_id, token_id, dec!(1000));
        assert!(result.is_ok());

        monitor.record_transaction(user_id, dec!(1000));

        let daily_volume = monitor.daily_volumes.get(&user_id).copied().unwrap();
        assert_eq!(daily_volume, dec!(1000));
    }

    #[test]
    fn test_suspicious_activity_detection() {
        let mut monitor = TransactionMonitor::new();
        let user_id = Uuid::new_v4();

        monitor.detect_suspicious_activity(
            user_id,
            vec![Uuid::new_v4()],
            "Unusual volume spike".to_string(),
            SuspiciousActivity::UnusualVolume,
            85,
        );

        let alerts = monitor.get_unreviewed_alerts();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].risk_score, 85);
    }

    #[test]
    fn test_alert_review() {
        let mut monitor = TransactionMonitor::new();
        let user_id = Uuid::new_v4();

        monitor.detect_suspicious_activity(
            user_id,
            vec![Uuid::new_v4()],
            "Test alert".to_string(),
            SuspiciousActivity::WashTrading,
            75,
        );

        let alert_id = monitor.alerts[0].id;

        let result = monitor.review_alert(alert_id, "Reviewed and cleared".to_string());
        assert!(result.is_ok());
        assert!(monitor.alerts[0].reviewed);
    }

    #[test]
    fn test_high_risk_alerts() {
        let mut monitor = TransactionMonitor::new();
        let user_id = Uuid::new_v4();

        monitor.detect_suspicious_activity(
            user_id,
            vec![Uuid::new_v4()],
            "Low risk".to_string(),
            SuspiciousActivity::FailedTransactions,
            50,
        );

        monitor.detect_suspicious_activity(
            user_id,
            vec![Uuid::new_v4()],
            "High risk".to_string(),
            SuspiciousActivity::FrontRunning,
            90,
        );

        let high_risk = monitor.get_high_risk_alerts(75);
        assert_eq!(high_risk.len(), 1);
        assert_eq!(high_risk[0].risk_score, 90);
    }

    #[test]
    fn test_compliance_report() {
        let mut monitor = TransactionMonitor::new();

        monitor.set_user_region(Uuid::new_v4(), Region::US);
        monitor.set_user_region(Uuid::new_v4(), Region::EU);
        monitor.set_user_region(Uuid::new_v4(), Region::US);

        monitor.detect_suspicious_activity(
            Uuid::new_v4(),
            vec![],
            "Test".to_string(),
            SuspiciousActivity::UnusualVolume,
            50,
        );

        let report = monitor.generate_report();

        assert_eq!(report.total_users, 3);
        assert_eq!(report.total_alerts, 1);
        assert_eq!(*report.users_by_region.get(&Region::US).unwrap(), 2);
        assert_eq!(*report.users_by_region.get(&Region::EU).unwrap(), 1);
    }

    #[test]
    fn test_volume_reset() {
        let mut monitor = TransactionMonitor::new();
        let user_id = Uuid::new_v4();

        monitor.record_transaction(user_id, dec!(1000));
        assert_eq!(
            monitor.daily_volumes.get(&user_id).copied().unwrap(),
            dec!(1000)
        );

        monitor.reset_daily_volumes();
        assert_eq!(
            monitor
                .daily_volumes
                .get(&user_id)
                .copied()
                .unwrap_or(Decimal::ZERO),
            Decimal::ZERO
        );
    }

    #[test]
    fn test_real_time_surveillance() {
        let mut surveillance = RealTimeSurveillance::new();

        let rule = SurveillanceRule {
            id: Uuid::new_v4(),
            name: "Volume Spike Detection".to_string(),
            rule_type: RuleType::VolumeSpike,
            enabled: true,
            threshold: Some(2.0),
        };

        surveillance.add_rule(rule);

        let violation = Violation {
            id: Uuid::new_v4(),
            rule_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            detected_at: Utc::now(),
            severity: ViolationSeverity::High,
            description: "Unusual volume detected".to_string(),
            evidence: vec!["Trade ID: 123".to_string()],
        };

        surveillance.record_violation(violation);

        let high_severity = surveillance.get_violations_by_severity(ViolationSeverity::High);
        assert_eq!(high_severity.len(), 1);
    }

    #[test]
    fn test_automated_sar() {
        let mut sar_system = AutomatedSAR::new();

        let sar_id = sar_system.create_sar(
            Uuid::new_v4(),
            SuspiciousActivity::UnusualVolume,
            "High volume spike detected".to_string(),
            Utc::now(),
            Utc::now(),
            dec!(100000),
            50,
        );

        assert_eq!(sar_system.get_pending_sars().len(), 1);

        let result = sar_system.submit_sar(sar_id);
        assert!(result.is_ok());
        assert_eq!(sar_system.get_pending_sars().len(), 0);
    }

    #[test]
    fn test_market_manipulation_detection() {
        let mut detector = MarketManipulationDetector::new(0.5);

        detector.add_price_point(Utc::now(), dec!(100));
        detector.add_price_point(Utc::now(), dec!(150));
        detector.add_price_point(Utc::now(), dec!(200));
        detector.add_price_point(Utc::now(), dec!(120));

        let alert = detector.detect_pump_and_dump(4);
        assert!(alert.is_some());

        if let Some(alert) = alert {
            assert_eq!(alert.alert_type, ManipulationType::PumpAndDump);
            assert!(alert.confidence > 0.0);
        }
    }

    #[test]
    fn test_wash_trading_detection() {
        let detector = MarketManipulationDetector::new(0.5);

        let user_id = Uuid::new_v4();
        let trades = vec![
            (user_id, user_id, dec!(100)), // Same buyer and seller
            (user_id, Uuid::new_v4(), dec!(200)),
            (user_id, user_id, dec!(150)), // Same buyer and seller
        ];

        let alert = detector.detect_wash_trading(&trades);
        assert!(alert.is_some());

        if let Some(alert) = alert {
            assert_eq!(alert.alert_type, ManipulationType::WashTrading);
        }
    }

    #[test]
    fn test_insider_trading_detection() {
        let mut detector = InsiderTradingDetector::new();

        let trade_time = Utc::now();
        let event_time = trade_time + chrono::Duration::hours(24);

        detector.detect_pre_event_trading(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            dec!(10000),
            trade_time,
            event_time,
            "Major announcement".to_string(),
        );

        let suspicious = detector.get_high_confidence_trades(0.5);
        assert_eq!(suspicious.len(), 1);
        assert!(suspicious[0].suspicion_score > 0.0);
    }
}
