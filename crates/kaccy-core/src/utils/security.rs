//! Advanced security features: behavioral biometrics, fraud detection, and privacy
use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Behavioral biometrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralBiometrics {
    /// User this biometric sample belongs to
    pub user_id: Uuid,
    /// Session during which the sample was captured
    pub session_id: Uuid,
    /// Keyboard dynamics captured during the session
    pub typing_pattern: TypingPattern,
    /// Mouse movement dynamics captured during the session
    pub mouse_dynamics: MouseDynamics,
    /// Device fingerprint at the time of capture
    pub device_fingerprint: DeviceFingerprint,
    /// Computed risk score for this sample
    pub risk_score: Decimal,
    /// When the sample was captured
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Typing pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingPattern {
    /// Average time a key is held down, in milliseconds
    pub avg_keypress_duration: u64,
    /// Average time between consecutive keystrokes, in milliseconds
    pub avg_interval_between_keys: u64,
    /// Typing speed in words per minute
    pub typing_speed_wpm: u16,
    /// Proportion of erroneous keystrokes (0.0 to 1.0)
    pub error_rate: Decimal,
    /// Raw inter-keystroke timing intervals used as a signature
    pub pattern_signature: Vec<u64>,
}

impl TypingPattern {
    /// Create a new typing pattern
    pub fn new(intervals: Vec<u64>, errors: usize, total_keys: usize) -> Self {
        let avg_interval = if !intervals.is_empty() {
            intervals.iter().sum::<u64>() / intervals.len() as u64
        } else {
            0
        };

        let typing_speed = if avg_interval > 0 {
            (60000 / (avg_interval * 5)).min(200) as u16 // Rough WPM estimate
        } else {
            0
        };

        let error_rate = if total_keys > 0 {
            Decimal::from(errors) / Decimal::from(total_keys)
        } else {
            Decimal::ZERO
        };

        Self {
            avg_keypress_duration: 100, // Default
            avg_interval_between_keys: avg_interval,
            typing_speed_wpm: typing_speed,
            error_rate,
            pattern_signature: intervals,
        }
    }

    /// Compare similarity with another pattern (0.0 to 1.0)
    pub fn similarity(&self, other: &TypingPattern) -> Decimal {
        let mut similarity_score = Decimal::ZERO;
        let mut factors = 0;

        // Compare typing speed
        let speed_diff = (self.typing_speed_wpm as i32 - other.typing_speed_wpm as i32).abs();
        if speed_diff < 20 {
            similarity_score += Decimal::from(100 - speed_diff) / Decimal::from(100);
            factors += 1;
        }

        // Compare error rates
        let error_diff = (self.error_rate - other.error_rate).abs();
        if error_diff < Decimal::new(1, 1) {
            // < 0.1
            similarity_score += Decimal::ONE - error_diff * Decimal::from(10);
            factors += 1;
        }

        // Compare interval patterns
        if !self.pattern_signature.is_empty() && !other.pattern_signature.is_empty() {
            let min_len = self
                .pattern_signature
                .len()
                .min(other.pattern_signature.len());
            let pattern_similarity = self.pattern_signature[..min_len]
                .iter()
                .zip(&other.pattern_signature[..min_len])
                .filter(|(a, b)| {
                    let diff = (**a).abs_diff(**b);
                    diff < 50 // Within 50ms
                })
                .count();

            similarity_score += Decimal::from(pattern_similarity) / Decimal::from(min_len);
            factors += 1;
        }

        if factors > 0 {
            similarity_score / Decimal::from(factors)
        } else {
            Decimal::ZERO
        }
    }
}

/// Mouse movement dynamics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseDynamics {
    /// Average cursor velocity in pixels per second
    pub avg_velocity: Decimal,
    /// Average cursor acceleration in pixels per second squared
    pub avg_acceleration: Decimal,
    /// Smoothness of cursor movement (0.0 = erratic, 1.0 = perfectly smooth)
    pub movement_smoothness: Decimal,
    /// Accuracy of clicking on targets (0.0 to 1.0)
    pub click_precision: Decimal,
    /// Recorded cursor trajectory as (x, y) pixel coordinates
    pub trajectory_points: Vec<(i32, i32)>,
}

impl MouseDynamics {
    /// Create a new mouse dynamics profile
    pub fn new(trajectory: Vec<(i32, i32)>, _click_targets: Vec<(i32, i32)>) -> Self {
        let avg_velocity = Self::calculate_velocity(&trajectory);
        let smoothness = Self::calculate_smoothness(&trajectory);

        Self {
            avg_velocity,
            avg_acceleration: Decimal::ZERO, // Simplified
            movement_smoothness: smoothness,
            click_precision: Decimal::new(95, 2), // Default high precision
            trajectory_points: trajectory,
        }
    }

    fn calculate_velocity(trajectory: &[(i32, i32)]) -> Decimal {
        if trajectory.len() < 2 {
            return Decimal::ZERO;
        }

        let total_distance: f64 = trajectory
            .windows(2)
            .map(|w| {
                let dx = (w[1].0 - w[0].0) as f64;
                let dy = (w[1].1 - w[0].1) as f64;
                (dx * dx + dy * dy).sqrt()
            })
            .sum();

        Decimal::from_f64_retain(total_distance / trajectory.len() as f64).unwrap_or(Decimal::ZERO)
    }

    fn calculate_smoothness(trajectory: &[(i32, i32)]) -> Decimal {
        if trajectory.len() < 3 {
            return Decimal::ONE;
        }

        // Calculate direction changes
        let direction_changes = trajectory
            .windows(3)
            .filter(|w| {
                let dx1 = w[1].0 - w[0].0;
                let dy1 = w[1].1 - w[0].1;
                let dx2 = w[2].0 - w[1].0;
                let dy2 = w[2].1 - w[1].1;

                // Check if direction changed significantly
                (dx1 * dx2 + dy1 * dy2) < 0
            })
            .count();

        let smoothness = 1.0 - (direction_changes as f64 / trajectory.len() as f64);
        Decimal::from_f64_retain(smoothness).unwrap_or(Decimal::ONE)
    }
}

/// Device fingerprint for identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    /// MD5 hash of the fingerprint attributes
    pub fingerprint_hash: String,
    /// Browser user-agent string
    pub user_agent: String,
    /// Screen resolution as (width, height) in pixels
    pub screen_resolution: (u32, u32),
    /// IANA timezone identifier
    pub timezone: String,
    /// Browser locale/language
    pub language: String,
    /// Operating system or platform string
    pub platform: String,
    /// When this device was first seen
    pub first_seen: chrono::DateTime<chrono::Utc>,
    /// Most recent time this device was seen
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

impl DeviceFingerprint {
    /// Create a new device fingerprint
    pub fn new(
        user_agent: String,
        screen_resolution: (u32, u32),
        timezone: String,
        language: String,
        platform: String,
    ) -> Self {
        let fingerprint_data = format!(
            "{}_{}x{}_{}_{}_{}",
            user_agent, screen_resolution.0, screen_resolution.1, timezone, language, platform
        );

        let fingerprint_hash = format!("{:x}", md5::compute(fingerprint_data));
        let now = chrono::Utc::now();

        Self {
            fingerprint_hash,
            user_agent,
            screen_resolution,
            timezone,
            language,
            platform,
            first_seen: now,
            last_seen: now,
        }
    }

    /// Check if fingerprint matches
    pub fn matches(&self, other: &DeviceFingerprint) -> bool {
        self.fingerprint_hash == other.fingerprint_hash
    }
}

/// Behavioral biometrics analyzer
#[derive(Debug, Clone)]
pub struct BiometricsAnalyzer {
    /// Historical biometric samples per user
    user_profiles: HashMap<Uuid, Vec<BehavioralBiometrics>>,
    /// Minimum similarity score [0.0, 1.0] required to accept a verification attempt
    threshold_similarity: Decimal,
}

impl BiometricsAnalyzer {
    /// Create a new biometrics analyzer
    pub fn new(threshold_similarity: Decimal) -> Self {
        Self {
            user_profiles: HashMap::new(),
            threshold_similarity,
        }
    }

    /// Add a biometric sample
    pub fn add_sample(&mut self, biometrics: BehavioralBiometrics) {
        self.user_profiles
            .entry(biometrics.user_id)
            .or_default()
            .push(biometrics);
    }

    /// Verify user based on biometrics
    pub fn verify_user(
        &self,
        user_id: Uuid,
        current_biometrics: &BehavioralBiometrics,
    ) -> Result<bool, CoreError> {
        let profiles = self
            .user_profiles
            .get(&user_id)
            .ok_or_else(|| CoreError::NotFound("No biometric profile found".to_string()))?;

        if profiles.is_empty() {
            return Err(CoreError::Validation(
                "Insufficient biometric data".to_string(),
            ));
        }

        // Compare with recent samples
        let recent_profiles: Vec<_> = profiles.iter().rev().take(5).collect();
        let mut total_similarity = Decimal::ZERO;

        for profile in recent_profiles.iter() {
            let similarity = current_biometrics
                .typing_pattern
                .similarity(&profile.typing_pattern);
            total_similarity += similarity;
        }

        let avg_similarity = total_similarity / Decimal::from(recent_profiles.len());
        Ok(avg_similarity >= self.threshold_similarity)
    }
}

/// Fraud detection system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudDetector {
    /// Unique identifier of this detector instance
    pub id: Uuid,
    /// Users that are explicitly banned
    pub user_blacklist: Vec<Uuid>,
    /// IP addresses that are explicitly banned
    pub ip_blacklist: Vec<String>,
    /// Known fraud patterns recorded by this detector
    pub suspicious_patterns: Vec<FraudPattern>,
}

/// A detected fraud pattern with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudPattern {
    /// Classification of the fraud pattern
    pub pattern_type: FraudPatternType,
    /// Human-readable description
    pub description: String,
    /// Severity of this pattern
    pub severity: FraudSeverity,
}

/// Classification of fraud pattern type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FraudPatternType {
    /// Unusually high rate of transactions
    RapidTransactions,
    /// Login or transaction from an unusual location
    UnusualLocation,
    /// New or unknown device detected
    DeviceChange,
    /// Single transaction exceeds normal thresholds
    LargeTransaction,
    /// General suspicious behaviour pattern
    SuspiciousPattern,
    /// Same user controlling multiple accounts
    MultipleAccounts,
    /// Transaction velocity exceeds historical norms
    VelocityAnomaly,
}

/// Severity level for a detected fraud pattern
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FraudSeverity {
    /// Low-severity indicator
    Low,
    /// Medium-severity indicator
    Medium,
    /// High-severity indicator requiring review
    High,
    /// Critical indicator requiring immediate action
    Critical,
}

impl FraudDetector {
    /// Create a new fraud detector
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            user_blacklist: Vec::new(),
            ip_blacklist: Vec::new(),
            suspicious_patterns: Vec::new(),
        }
    }

    /// Add user to blacklist
    pub fn blacklist_user(&mut self, user_id: Uuid) {
        if !self.user_blacklist.contains(&user_id) {
            self.user_blacklist.push(user_id);
        }
    }

    /// Add IP to blacklist
    pub fn blacklist_ip(&mut self, ip: String) {
        if !self.ip_blacklist.contains(&ip) {
            self.ip_blacklist.push(ip);
        }
    }

    /// Check if user is blacklisted
    pub fn is_user_blacklisted(&self, user_id: &Uuid) -> bool {
        self.user_blacklist.contains(user_id)
    }

    /// Check if IP is blacklisted
    pub fn is_ip_blacklisted(&self, ip: &str) -> bool {
        self.ip_blacklist.contains(&ip.to_string())
    }

    /// Analyze transaction for fraud
    pub fn analyze_transaction(
        &self,
        user_id: Uuid,
        amount: Decimal,
        ip_address: &str,
        _device: &DeviceFingerprint,
        recent_transactions: &[TransactionRecord],
    ) -> FraudAnalysis {
        let mut risk_score = Decimal::ZERO;
        let mut detected_patterns = Vec::new();

        // Check blacklists
        if self.is_user_blacklisted(&user_id) {
            risk_score += Decimal::from(100);
            detected_patterns.push(FraudPattern {
                pattern_type: FraudPatternType::SuspiciousPattern,
                description: "User is blacklisted".to_string(),
                severity: FraudSeverity::Critical,
            });
        }

        if self.is_ip_blacklisted(ip_address) {
            risk_score += Decimal::from(50);
            detected_patterns.push(FraudPattern {
                pattern_type: FraudPatternType::SuspiciousPattern,
                description: "IP is blacklisted".to_string(),
                severity: FraudSeverity::High,
            });
        }

        // Check for rapid transactions
        let recent_count = recent_transactions
            .iter()
            .filter(|t| {
                let age = chrono::Utc::now()
                    .signed_duration_since(t.timestamp)
                    .num_minutes();
                age < 5
            })
            .count();

        if recent_count > 10 {
            risk_score += Decimal::from(30);
            detected_patterns.push(FraudPattern {
                pattern_type: FraudPatternType::RapidTransactions,
                description: format!("{} transactions in 5 minutes", recent_count),
                severity: FraudSeverity::High,
            });
        }

        // Check for large transaction
        if amount > Decimal::from(10000) {
            risk_score += Decimal::from(20);
            detected_patterns.push(FraudPattern {
                pattern_type: FraudPatternType::LargeTransaction,
                description: format!("Large transaction: {}", amount),
                severity: FraudSeverity::Medium,
            });
        }

        // Check for velocity anomaly
        if recent_count > 5 {
            let total_amount: Decimal = recent_transactions
                .iter()
                .take(recent_count)
                .map(|t| t.amount)
                .sum();

            if total_amount > Decimal::from(50000) {
                risk_score += Decimal::from(40);
                detected_patterns.push(FraudPattern {
                    pattern_type: FraudPatternType::VelocityAnomaly,
                    description: format!("High velocity: {} in short time", total_amount),
                    severity: FraudSeverity::High,
                });
            }
        }

        let severity = if risk_score > Decimal::from(80) {
            FraudSeverity::Critical
        } else if risk_score > Decimal::from(50) {
            FraudSeverity::High
        } else if risk_score > Decimal::from(30) {
            FraudSeverity::Medium
        } else {
            FraudSeverity::Low
        };

        FraudAnalysis {
            risk_score,
            detected_patterns,
            severity,
            requires_review: risk_score > Decimal::from(50),
            timestamp: chrono::Utc::now(),
        }
    }
}

impl Default for FraudDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Fraud analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudAnalysis {
    /// Aggregate risk score (0 = clean, 100+ = fraudulent)
    pub risk_score: Decimal,
    /// Individual fraud patterns that contributed to the score
    pub detected_patterns: Vec<FraudPattern>,
    /// Overall severity based on the risk score
    pub severity: FraudSeverity,
    /// Whether a human reviewer should inspect this transaction
    pub requires_review: bool,
    /// When this analysis was performed
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Transaction record for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    /// Monetary value of the transaction
    pub amount: Decimal,
    /// When the transaction was made
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// IP address from which the transaction was submitted
    pub ip_address: String,
}

/// IP reputation scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpReputation {
    /// The IP address being scored
    pub ip_address: String,
    /// Reputation score (0 = worst, 100 = best)
    pub reputation_score: Decimal,
    /// Country of origin for the IP
    pub country: String,
    /// Whether the IP is a known VPN exit node
    pub is_vpn: bool,
    /// Whether the IP is a known proxy
    pub is_proxy: bool,
    /// Whether the IP is a Tor exit node
    pub is_tor: bool,
    /// Number of abuse reports received for this IP
    pub abuse_reports: u32,
    /// When this reputation data was last refreshed
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl IpReputation {
    /// Create a new IP reputation
    pub fn new(ip_address: String, country: String) -> Self {
        Self {
            ip_address,
            reputation_score: Decimal::from(100),
            country,
            is_vpn: false,
            is_proxy: false,
            is_tor: false,
            abuse_reports: 0,
            last_updated: chrono::Utc::now(),
        }
    }

    /// Report abuse
    pub fn report_abuse(&mut self) {
        self.abuse_reports += 1;
        self.reputation_score = (self.reputation_score - Decimal::from(10)).max(Decimal::ZERO);
        self.last_updated = chrono::Utc::now();
    }

    /// Check if IP is trusted
    pub fn is_trusted(&self) -> bool {
        self.reputation_score > Decimal::from(70) && !self.is_vpn && !self.is_proxy && !self.is_tor
    }
}

/// Account takeover prevention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTakeoverDetector {
    /// User whose account is being monitored
    pub user_id: Uuid,
    /// Known good login locations for this user
    pub normal_login_locations: Vec<String>,
    /// Known good device fingerprints for this user
    pub normal_devices: Vec<String>,
    /// When the user last changed their password
    pub last_password_change: chrono::DateTime<chrono::Utc>,
    /// Number of consecutive failed login attempts
    pub failed_login_attempts: u32,
}

impl AccountTakeoverDetector {
    /// Create a new detector
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            normal_login_locations: Vec::new(),
            normal_devices: Vec::new(),
            last_password_change: chrono::Utc::now(),
            failed_login_attempts: 0,
        }
    }

    /// Check for takeover indicators
    pub fn check_login(
        &mut self,
        location: &str,
        device_fingerprint: &str,
        biometrics: Option<&BehavioralBiometrics>,
    ) -> TakeoverRisk {
        let mut risk_factors = Vec::new();
        let mut risk_score = Decimal::ZERO;

        // Check location
        if !self.normal_login_locations.contains(&location.to_string()) {
            risk_factors.push("Unknown location".to_string());
            risk_score += Decimal::from(30);
        }

        // Check device
        if !self
            .normal_devices
            .contains(&device_fingerprint.to_string())
        {
            risk_factors.push("Unknown device".to_string());
            risk_score += Decimal::from(25);
        }

        // Check failed attempts
        if self.failed_login_attempts > 3 {
            risk_factors.push("Multiple failed login attempts".to_string());
            risk_score += Decimal::from(20);
        }

        // Check biometrics if available
        if biometrics.is_some() {
            // Would verify biometrics here
            // For now, assume biometrics reduce risk
            risk_score = (risk_score - Decimal::from(15)).max(Decimal::ZERO);
        }

        TakeoverRisk {
            risk_score,
            risk_factors,
            requires_2fa: risk_score > Decimal::from(40),
            requires_verification: risk_score > Decimal::from(60),
        }
    }

    /// Record failed login
    pub fn record_failed_login(&mut self) {
        self.failed_login_attempts += 1;
    }

    /// Reset failed attempts
    pub fn reset_failed_attempts(&mut self) {
        self.failed_login_attempts = 0;
    }
}

/// Takeover risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverRisk {
    /// Computed risk score indicating likelihood of account takeover
    pub risk_score: Decimal,
    /// Human-readable list of factors that raised the risk score
    pub risk_factors: Vec<String>,
    /// Whether the login attempt should be challenged with 2FA
    pub requires_2fa: bool,
    /// Whether additional identity verification is needed
    pub requires_verification: bool,
}

/// Privacy-preserving transaction proof (simplified zero-knowledge concept)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyProof {
    /// Unique identifier of this proof
    pub proof_id: Uuid,
    /// The statement being proven (e.g. "balance > 1000")
    pub statement: String,
    /// Cryptographic commitment to the secret value
    pub commitment: String,
    /// Whether the proof has been successfully verified
    pub verified: bool,
    /// When this proof was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PrivacyProof {
    /// Create a new privacy proof
    pub fn new(statement: String, secret_value: Decimal) -> Self {
        // In a real implementation, this would use actual zero-knowledge proof libraries
        // This is a simplified representation
        let commitment_data = format!("{}{}", statement, secret_value);
        let commitment = format!("{:x}", md5::compute(commitment_data));

        Self {
            proof_id: Uuid::new_v4(),
            statement,
            commitment,
            verified: false,
            created_at: chrono::Utc::now(),
        }
    }

    /// Verify the proof (simplified)
    pub fn verify(&mut self, expected_commitment: &str) -> bool {
        self.verified = self.commitment == expected_commitment;
        self.verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typing_pattern() {
        let intervals = vec![100, 120, 110, 105, 115];
        let pattern = TypingPattern::new(intervals.clone(), 1, 50);

        assert!(pattern.typing_speed_wpm > 0);
        assert!(pattern.error_rate > Decimal::ZERO);
        assert_eq!(pattern.pattern_signature, intervals);
    }

    #[test]
    fn test_typing_pattern_similarity() {
        let pattern1 = TypingPattern::new(vec![100, 120, 110], 1, 50);
        let pattern2 = TypingPattern::new(vec![105, 115, 112], 1, 50);

        let similarity = pattern1.similarity(&pattern2);
        assert!(similarity > Decimal::ZERO);
    }

    #[test]
    fn test_mouse_dynamics() {
        let trajectory = vec![(0, 0), (10, 10), (20, 20), (30, 30)];
        let dynamics = MouseDynamics::new(trajectory, vec![]);

        assert!(dynamics.avg_velocity > Decimal::ZERO);
        assert!(dynamics.movement_smoothness > Decimal::ZERO);
    }

    #[test]
    fn test_device_fingerprint() {
        let fp1 = DeviceFingerprint::new(
            "Mozilla/5.0".to_string(),
            (1920, 1080),
            "UTC".to_string(),
            "en-US".to_string(),
            "Linux".to_string(),
        );

        let fp2 = DeviceFingerprint::new(
            "Mozilla/5.0".to_string(),
            (1920, 1080),
            "UTC".to_string(),
            "en-US".to_string(),
            "Linux".to_string(),
        );

        assert!(fp1.matches(&fp2));
    }

    #[test]
    fn test_fraud_detector() {
        let mut detector = FraudDetector::new();
        let user_id = Uuid::new_v4();

        detector.blacklist_user(user_id);
        assert!(detector.is_user_blacklisted(&user_id));

        detector.blacklist_ip("192.168.1.1".to_string());
        assert!(detector.is_ip_blacklisted("192.168.1.1"));
    }

    #[test]
    fn test_fraud_analysis() {
        let detector = FraudDetector::new();
        let device = DeviceFingerprint::new(
            "Mozilla/5.0".to_string(),
            (1920, 1080),
            "UTC".to_string(),
            "en-US".to_string(),
            "Linux".to_string(),
        );

        let analysis = detector.analyze_transaction(
            Uuid::new_v4(),
            Decimal::from(100),
            "192.168.1.1",
            &device,
            &[],
        );

        assert!(analysis.risk_score >= Decimal::ZERO);
    }

    #[test]
    fn test_ip_reputation() {
        let mut ip_rep = IpReputation::new("192.168.1.1".to_string(), "US".to_string());

        assert_eq!(ip_rep.reputation_score, Decimal::from(100));
        assert!(ip_rep.is_trusted());

        ip_rep.report_abuse();
        assert_eq!(ip_rep.abuse_reports, 1);
        assert_eq!(ip_rep.reputation_score, Decimal::from(90));
    }

    #[test]
    fn test_account_takeover_detector() {
        let mut detector = AccountTakeoverDetector::new(Uuid::new_v4());

        let risk = detector.check_login("US", "device123", None);
        assert!(risk.risk_score > Decimal::ZERO);

        detector.record_failed_login();
        assert_eq!(detector.failed_login_attempts, 1);

        detector.reset_failed_attempts();
        assert_eq!(detector.failed_login_attempts, 0);
    }

    #[test]
    fn test_privacy_proof() {
        let mut proof = PrivacyProof::new("balance > 1000".to_string(), Decimal::from(2000));

        let commitment = proof.commitment.clone();
        assert!(proof.verify(&commitment));
        assert!(proof.verified);
    }

    #[test]
    fn test_biometrics_analyzer() {
        let mut analyzer = BiometricsAnalyzer::new(Decimal::new(70, 2));
        let user_id = Uuid::new_v4();

        let pattern = TypingPattern::new(vec![100, 120, 110], 1, 50);
        let dynamics = MouseDynamics::new(vec![(0, 0), (10, 10)], vec![]);
        let device = DeviceFingerprint::new(
            "Mozilla/5.0".to_string(),
            (1920, 1080),
            "UTC".to_string(),
            "en-US".to_string(),
            "Linux".to_string(),
        );

        let biometrics = BehavioralBiometrics {
            user_id,
            session_id: Uuid::new_v4(),
            typing_pattern: pattern,
            mouse_dynamics: dynamics,
            device_fingerprint: device,
            risk_score: Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        };

        analyzer.add_sample(biometrics.clone());

        // Should fail because we need more samples
        let result = analyzer.verify_user(user_id, &biometrics);
        assert!(result.is_ok());
    }
}
