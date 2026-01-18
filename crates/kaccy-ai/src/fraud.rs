//! Advanced fraud detection module
//!
//! This module provides sophisticated fraud detection capabilities including:
//! - Sybil attack detection (fake account networks)
//! - Image manipulation detection
//! - Wash trading detection
//! - Behavioral anomaly detection

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::error::{AiError, Result};
use crate::llm::{ChatRequest, LlmClient};

/// Fraud detector trait for different detection strategies
#[async_trait]
pub trait FraudDetector: Send + Sync {
    /// Analyze for fraud indicators
    async fn analyze(&self, data: &FraudAnalysisInput) -> Result<FraudAnalysisResult>;

    /// Get detector name
    fn name(&self) -> &str;
}

/// Input data for fraud analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudAnalysisInput {
    /// User ID being analyzed
    pub user_id: String,
    /// Account creation timestamp (ISO 8601)
    pub account_created: String,
    /// User's email domain
    pub email_domain: Option<String>,
    /// IP addresses used
    pub ip_addresses: Vec<String>,
    /// User agent strings
    pub user_agents: Vec<String>,
    /// Trading history
    pub trades: Vec<TradeRecord>,
    /// Related accounts (same IP, similar patterns)
    pub related_accounts: Vec<RelatedAccount>,
    /// Commitment history
    pub commitments: Vec<CommitmentRecord>,
    /// Evidence submissions
    pub evidence_urls: Vec<String>,
}

/// Trade record for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Trade ID
    pub trade_id: String,
    /// Token involved
    pub token_id: String,
    /// Trade type (buy/sell)
    pub trade_type: String,
    /// Amount in token units
    pub amount: f64,
    /// Price in BTC
    pub price_btc: f64,
    /// Counterparty user ID (if known)
    pub counterparty: Option<String>,
    /// Timestamp
    pub timestamp: String,
}

/// Related account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedAccount {
    /// Related user ID
    pub user_id: String,
    /// Relationship type
    pub relationship: RelationshipType,
    /// Similarity score (0-1)
    pub similarity: f64,
    /// Evidence for relationship
    pub evidence: Vec<String>,
}

/// Types of relationships between accounts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    /// Same IP address
    SameIp,
    /// Same device fingerprint
    SameDevice,
    /// Similar email pattern
    SimilarEmail,
    /// Trading partners
    TradingPartner,
    /// Similar behavior
    SimilarBehavior,
    /// Referral chain
    ReferralChain,
}

/// Commitment record for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentRecord {
    /// Commitment ID
    pub commitment_id: String,
    /// Status
    pub status: String,
    /// Quality score (if verified)
    pub quality_score: Option<f64>,
    /// Time to completion (hours)
    pub completion_hours: Option<f64>,
}

/// Result of fraud analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudAnalysisResult {
    /// Overall risk level
    pub risk_level: RiskLevel,
    /// Risk score (0-100)
    pub risk_score: u32,
    /// Detected fraud types
    pub detected_types: Vec<FraudType>,
    /// Detailed findings
    pub findings: Vec<FraudFinding>,
    /// Recommended actions
    pub recommendations: Vec<String>,
    /// Confidence in the analysis (0-1)
    pub confidence: f64,
}

/// Risk levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// No significant risk detected
    Low,
    /// Some suspicious indicators
    Medium,
    /// Strong fraud indicators
    High,
    /// Confirmed fraud patterns
    Critical,
}

impl RiskLevel {
    /// Get from score
    #[must_use]
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=25 => RiskLevel::Low,
            26..=50 => RiskLevel::Medium,
            51..=75 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    /// Get color code for UI
    #[must_use]
    pub fn color(&self) -> &'static str {
        match self {
            RiskLevel::Low => "green",
            RiskLevel::Medium => "yellow",
            RiskLevel::High => "orange",
            RiskLevel::Critical => "red",
        }
    }
}

/// Types of fraud detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FraudType {
    /// Multiple fake accounts
    SybilAttack,
    /// Manipulated evidence images
    ImageManipulation,
    /// Self-trading to fake volume
    WashTrading,
    /// Fake commitment completion
    FakeEvidence,
    /// Gaming the reputation system
    ReputationGaming,
    /// Unusual transaction patterns
    AnomalousTrading,
    /// Bot-like behavior
    BotActivity,
    /// Identity fraud
    IdentityFraud,
}

impl FraudType {
    /// Get severity weight
    #[must_use]
    pub fn severity(&self) -> u32 {
        match self {
            FraudType::SybilAttack => 30,
            FraudType::WashTrading => 25,
            FraudType::ImageManipulation => 20,
            FraudType::FakeEvidence => 25,
            FraudType::ReputationGaming => 15,
            FraudType::AnomalousTrading => 10,
            FraudType::BotActivity => 10,
            FraudType::IdentityFraud => 35,
        }
    }

    /// Get description
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            FraudType::SybilAttack => "Multiple accounts controlled by the same entity",
            FraudType::WashTrading => "Trading with oneself to fake volume/activity",
            FraudType::ImageManipulation => "Evidence images have been digitally altered",
            FraudType::FakeEvidence => "Evidence does not support claimed completion",
            FraudType::ReputationGaming => "Artificially inflating reputation score",
            FraudType::AnomalousTrading => "Unusual trading patterns detected",
            FraudType::BotActivity => "Automated non-human activity detected",
            FraudType::IdentityFraud => "Using false identity information",
        }
    }
}

/// Detailed fraud finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudFinding {
    /// Type of fraud
    pub fraud_type: FraudType,
    /// Confidence (0-1)
    pub confidence: f64,
    /// Detailed description
    pub description: String,
    /// Supporting evidence
    pub evidence: Vec<String>,
    /// Severity (1-10)
    pub severity: u32,
}

/// Sybil attack detector
pub struct SybilDetector {
    /// Minimum similarity threshold for flagging
    similarity_threshold: f64,
    /// Minimum cluster size to flag
    min_cluster_size: usize,
}

impl SybilDetector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            similarity_threshold: 0.7,
            min_cluster_size: 3,
        }
    }

    #[must_use]
    pub fn with_config(similarity_threshold: f64, min_cluster_size: usize) -> Self {
        Self {
            similarity_threshold,
            min_cluster_size,
        }
    }

    /// Analyze IP address patterns
    fn analyze_ip_patterns(&self, input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        // Check for multiple accounts from same IP
        let same_ip_accounts: Vec<_> = input
            .related_accounts
            .iter()
            .filter(|a| a.relationship == RelationshipType::SameIp)
            .collect();

        if same_ip_accounts.len() >= self.min_cluster_size {
            findings.push(FraudFinding {
                fraud_type: FraudType::SybilAttack,
                confidence: 0.8,
                description: format!(
                    "{} accounts share the same IP address",
                    same_ip_accounts.len() + 1
                ),
                evidence: same_ip_accounts
                    .iter()
                    .map(|a| format!("Account {} (similarity: {:.2})", a.user_id, a.similarity))
                    .collect(),
                severity: 8,
            });
        }

        findings
    }

    /// Analyze email patterns
    fn analyze_email_patterns(&self, input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        let similar_email_accounts: Vec<_> = input
            .related_accounts
            .iter()
            .filter(|a| {
                a.relationship == RelationshipType::SimilarEmail
                    && a.similarity >= self.similarity_threshold
            })
            .collect();

        if similar_email_accounts.len() >= 2 {
            findings.push(FraudFinding {
                fraud_type: FraudType::SybilAttack,
                confidence: 0.6,
                description: "Multiple accounts with similar email patterns".to_string(),
                evidence: similar_email_accounts
                    .iter()
                    .map(|a| {
                        format!(
                            "Account {} (pattern similarity: {:.2})",
                            a.user_id, a.similarity
                        )
                    })
                    .collect(),
                severity: 5,
            });
        }

        findings
    }

    /// Analyze behavioral patterns
    fn analyze_behavioral_patterns(&self, input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        let similar_behavior: Vec<_> = input
            .related_accounts
            .iter()
            .filter(|a| {
                a.relationship == RelationshipType::SimilarBehavior
                    && a.similarity >= self.similarity_threshold
            })
            .collect();

        if similar_behavior.len() >= self.min_cluster_size {
            findings.push(FraudFinding {
                fraud_type: FraudType::SybilAttack,
                confidence: 0.7,
                description: "Coordinated behavior detected across multiple accounts".to_string(),
                evidence: similar_behavior
                    .iter()
                    .map(|a| {
                        format!(
                            "Account {} shows {:.0}% similar behavior",
                            a.user_id,
                            a.similarity * 100.0
                        )
                    })
                    .collect(),
                severity: 7,
            });
        }

        findings
    }
}

impl Default for SybilDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FraudDetector for SybilDetector {
    async fn analyze(&self, input: &FraudAnalysisInput) -> Result<FraudAnalysisResult> {
        let mut findings = Vec::new();

        findings.extend(self.analyze_ip_patterns(input));
        findings.extend(self.analyze_email_patterns(input));
        findings.extend(self.analyze_behavioral_patterns(input));

        let risk_score = findings
            .iter()
            .map(|f| f.severity * 3)
            .sum::<u32>()
            .min(100);

        let detected_types: HashSet<_> = findings.iter().map(|f| f.fraud_type).collect();

        let recommendations = if risk_score > 50 {
            vec![
                "Require additional identity verification".to_string(),
                "Monitor account activity closely".to_string(),
                "Consider restricting token issuance".to_string(),
            ]
        } else if risk_score > 25 {
            vec!["Flag for manual review".to_string()]
        } else {
            vec![]
        };

        Ok(FraudAnalysisResult {
            risk_level: RiskLevel::from_score(risk_score),
            risk_score,
            detected_types: detected_types.into_iter().collect(),
            findings,
            recommendations,
            confidence: 0.8,
        })
    }

    fn name(&self) -> &'static str {
        "sybil_detector"
    }
}

/// Wash trading detector
pub struct WashTradingDetector {
    /// Minimum trade count to analyze
    min_trades: usize,
    /// Self-trade ratio threshold
    self_trade_threshold: f64,
    /// Suspicious pattern threshold
    #[allow(dead_code)]
    pattern_threshold: f64,
}

impl WashTradingDetector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_trades: 5,
            self_trade_threshold: 0.3,
            pattern_threshold: 0.7,
        }
    }

    /// Detect self-trading
    fn detect_self_trading(&self, input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        // Count trades with related accounts
        let related_ids: HashSet<_> = input.related_accounts.iter().map(|a| &a.user_id).collect();

        let trades_with_related: Vec<_> = input
            .trades
            .iter()
            .filter(|t| {
                t.counterparty
                    .as_ref()
                    .is_some_and(|c| related_ids.contains(c))
            })
            .collect();

        if !input.trades.is_empty() {
            let related_ratio = trades_with_related.len() as f64 / input.trades.len() as f64;

            if related_ratio >= self.self_trade_threshold && input.trades.len() >= self.min_trades {
                findings.push(FraudFinding {
                    fraud_type: FraudType::WashTrading,
                    confidence: related_ratio,
                    description: format!(
                        "{:.0}% of trades are with related accounts",
                        related_ratio * 100.0
                    ),
                    evidence: trades_with_related
                        .iter()
                        .take(5)
                        .map(|t| {
                            format!(
                                "Trade {} with {}",
                                t.trade_id,
                                t.counterparty.as_deref().unwrap_or("unknown")
                            )
                        })
                        .collect(),
                    severity: 8,
                });
            }
        }

        findings
    }

    /// Detect round-trip patterns (buy then sell same amount)
    fn detect_round_trips(input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        // Group trades by token
        let mut token_trades: HashMap<&str, Vec<&TradeRecord>> = HashMap::new();
        for trade in &input.trades {
            token_trades.entry(&trade.token_id).or_default().push(trade);
        }

        for (token_id, trades) in token_trades {
            // Look for buy-sell pairs with similar amounts
            let buys: Vec<_> = trades.iter().filter(|t| t.trade_type == "buy").collect();
            let sells: Vec<_> = trades.iter().filter(|t| t.trade_type == "sell").collect();

            let mut round_trips = 0;
            for buy in &buys {
                for sell in &sells {
                    // Check if amounts are similar (within 5%)
                    let diff = (buy.amount - sell.amount).abs() / buy.amount;
                    if diff < 0.05 {
                        round_trips += 1;
                    }
                }
            }

            let total_pairs = buys.len().min(sells.len());
            if total_pairs >= 3 && f64::from(round_trips) / total_pairs as f64 > 0.5 {
                findings.push(FraudFinding {
                    fraud_type: FraudType::WashTrading,
                    confidence: 0.7,
                    description: format!(
                        "Multiple round-trip trades detected for token {token_id}"
                    ),
                    evidence: vec![format!(
                        "{} potential round-trips out of {} buy-sell pairs",
                        round_trips, total_pairs
                    )],
                    severity: 7,
                });
            }
        }

        findings
    }

    /// Detect price manipulation patterns
    fn detect_price_manipulation(input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        // Group trades by token and check for suspicious patterns
        let mut token_trades: HashMap<&str, Vec<&TradeRecord>> = HashMap::new();
        for trade in &input.trades {
            token_trades.entry(&trade.token_id).or_default().push(trade);
        }

        for (token_id, mut trades) in token_trades {
            if trades.len() < 5 {
                continue;
            }

            // Sort by timestamp
            trades.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            // Check for rapid price changes
            let prices: Vec<f64> = trades.iter().map(|t| t.price_btc).collect();
            let mut large_swings = 0;

            for window in prices.windows(3) {
                let change1 = (window[1] - window[0]).abs() / window[0];
                let change2 = (window[2] - window[1]).abs() / window[1];

                // If both changes are > 10% in opposite directions
                if change1 > 0.1 && change2 > 0.1 {
                    let direction1 = window[1] > window[0];
                    let direction2 = window[2] > window[1];
                    if direction1 != direction2 {
                        large_swings += 1;
                    }
                }
            }

            if large_swings >= 2 {
                findings.push(FraudFinding {
                    fraud_type: FraudType::AnomalousTrading,
                    confidence: 0.6,
                    description: format!(
                        "Price manipulation pattern detected for token {token_id}"
                    ),
                    evidence: vec![format!("{} rapid price reversals detected", large_swings)],
                    severity: 6,
                });
            }
        }

        findings
    }
}

impl Default for WashTradingDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FraudDetector for WashTradingDetector {
    async fn analyze(&self, input: &FraudAnalysisInput) -> Result<FraudAnalysisResult> {
        let mut findings = Vec::new();

        if input.trades.len() >= self.min_trades {
            findings.extend(self.detect_self_trading(input));
            findings.extend(Self::detect_round_trips(input));
            findings.extend(Self::detect_price_manipulation(input));
        }

        let risk_score = findings
            .iter()
            .map(|f| f.severity * 3)
            .sum::<u32>()
            .min(100);

        let detected_types: HashSet<_> = findings.iter().map(|f| f.fraud_type).collect();

        let recommendations = if risk_score > 50 {
            vec![
                "Freeze trading activity pending investigation".to_string(),
                "Review all trades for potential refunds".to_string(),
                "Consider permanent trading ban".to_string(),
            ]
        } else if risk_score > 25 {
            vec![
                "Monitor trading activity closely".to_string(),
                "Set trading volume limits".to_string(),
            ]
        } else {
            vec![]
        };

        Ok(FraudAnalysisResult {
            risk_level: RiskLevel::from_score(risk_score),
            risk_score,
            detected_types: detected_types.into_iter().collect(),
            findings,
            recommendations,
            confidence: 0.75,
        })
    }

    fn name(&self) -> &'static str {
        "wash_trading_detector"
    }
}

/// Image manipulation detector using LLM vision
pub struct ImageManipulationDetector {
    llm: LlmClient,
}

impl ImageManipulationDetector {
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Analyze image for manipulation
    pub async fn analyze_image(&self, image_url: &str) -> Result<ImageAnalysisResult> {
        let prompt = r#"Analyze this image for signs of digital manipulation or forgery.

Check for:
1. Inconsistent lighting or shadows
2. Clone/copy-paste artifacts
3. Blur or sharpness inconsistencies
4. Metadata anomalies (if visible)
5. Text that appears edited
6. Misaligned elements
7. Compression artifacts in unusual patterns

Respond in JSON format:
{
    "is_manipulated": <boolean>,
    "confidence": <0.0-1.0>,
    "manipulation_type": "<none|editing|compositing|generation|screenshot_editing|text_modification>",
    "indicators": ["<list of specific indicators found>"],
    "affected_regions": ["<list of regions showing manipulation>"],
    "analysis": "<detailed explanation>"
}"#;

        let request = ChatRequest::with_vision(
            "You are an expert digital forensics analyst specializing in image manipulation detection.",
            prompt,
            image_url,
        )
        .max_tokens(1024)
        .temperature(0.2);

        let response = self.llm.chat(request).await?;

        self.parse_image_analysis(&response.message.content)
    }

    fn parse_image_analysis(&self, response: &str) -> Result<ImageAnalysisResult> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        serde_json::from_str(json_str)
            .map_err(|e| AiError::EvaluationFailed(format!("Failed to parse image analysis: {e}")))
    }
}

/// Result of image manipulation analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAnalysisResult {
    /// Whether manipulation was detected
    pub is_manipulated: bool,
    /// Confidence in the detection (0-1)
    pub confidence: f64,
    /// Type of manipulation detected
    pub manipulation_type: String,
    /// Specific indicators found
    pub indicators: Vec<String>,
    /// Regions showing manipulation
    pub affected_regions: Vec<String>,
    /// Detailed analysis
    pub analysis: String,
}

/// Reputation gaming detector
pub struct ReputationGamingDetector;

impl ReputationGamingDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Analyze commitment patterns for gaming
    fn analyze_commitment_patterns(&self, input: &FraudAnalysisInput) -> Vec<FraudFinding> {
        let mut findings = Vec::new();

        if input.commitments.len() < 5 {
            return findings;
        }

        // Check for suspiciously fast completions
        let fast_completions: Vec<_> = input
            .commitments
            .iter()
            .filter(|c| c.status == "verified" && c.completion_hours.is_some_and(|h| h < 1.0))
            .collect();

        let fast_ratio = fast_completions.len() as f64 / input.commitments.len() as f64;
        if fast_ratio > 0.5 && fast_completions.len() >= 3 {
            findings.push(FraudFinding {
                fraud_type: FraudType::ReputationGaming,
                confidence: 0.6,
                description: "Unusually fast commitment completions".to_string(),
                evidence: vec![format!(
                    "{:.0}% of commitments completed in under 1 hour",
                    fast_ratio * 100.0
                )],
                severity: 5,
            });
        }

        // Check for uniform quality scores (suspiciously consistent)
        let verified: Vec<_> = input
            .commitments
            .iter()
            .filter_map(|c| c.quality_score)
            .collect();

        if verified.len() >= 5 {
            let avg = verified.iter().sum::<f64>() / verified.len() as f64;
            let variance =
                verified.iter().map(|&s| (s - avg).powi(2)).sum::<f64>() / verified.len() as f64;

            // Very low variance with high scores is suspicious
            if variance < 5.0 && avg > 90.0 {
                findings.push(FraudFinding {
                    fraud_type: FraudType::ReputationGaming,
                    confidence: 0.5,
                    description: "Suspiciously consistent high quality scores".to_string(),
                    evidence: vec![format!(
                        "Average score: {:.1}, variance: {:.2}",
                        avg, variance
                    )],
                    severity: 4,
                });
            }
        }

        findings
    }
}

impl Default for ReputationGamingDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FraudDetector for ReputationGamingDetector {
    async fn analyze(&self, input: &FraudAnalysisInput) -> Result<FraudAnalysisResult> {
        let findings = self.analyze_commitment_patterns(input);

        let risk_score = findings
            .iter()
            .map(|f| f.severity * 4)
            .sum::<u32>()
            .min(100);

        let detected_types: HashSet<_> = findings.iter().map(|f| f.fraud_type).collect();

        let recommendations = if risk_score > 40 {
            vec![
                "Require more detailed evidence for commitments".to_string(),
                "Extend verification review period".to_string(),
            ]
        } else {
            vec![]
        };

        Ok(FraudAnalysisResult {
            risk_level: RiskLevel::from_score(risk_score),
            risk_score,
            detected_types: detected_types.into_iter().collect(),
            findings,
            recommendations,
            confidence: 0.65,
        })
    }

    fn name(&self) -> &'static str {
        "reputation_gaming_detector"
    }
}

/// Comprehensive fraud analysis service
pub struct FraudAnalysisService {
    detectors: Vec<Box<dyn FraudDetector>>,
}

impl FraudAnalysisService {
    /// Create with default detectors
    #[must_use]
    pub fn new() -> Self {
        Self {
            detectors: vec![
                Box::new(SybilDetector::new()),
                Box::new(WashTradingDetector::new()),
                Box::new(ReputationGamingDetector::new()),
            ],
        }
    }

    /// Create with LLM support for image analysis
    #[must_use]
    pub fn with_llm(llm: LlmClient) -> Self {
        let service = Self::new();
        // Note: ImageManipulationDetector is used separately for specific image analysis
        // as it requires image URLs rather than the standard FraudAnalysisInput
        let _ = llm; // LLM would be used for image detection when needed
        service
    }

    /// Add a custom detector
    #[must_use]
    pub fn add_detector(mut self, detector: Box<dyn FraudDetector>) -> Self {
        self.detectors.push(detector);
        self
    }

    /// Run comprehensive fraud analysis
    pub async fn analyze(&self, input: &FraudAnalysisInput) -> Result<ComprehensiveFraudReport> {
        let mut all_results = Vec::new();

        for detector in &self.detectors {
            match detector.analyze(input).await {
                Ok(result) => all_results.push((detector.name().to_string(), result)),
                Err(e) => {
                    tracing::warn!(detector = detector.name(), error = %e, "Detector failed");
                }
            }
        }

        // Aggregate results
        let mut combined_score = 0u32;
        let mut all_findings = Vec::new();
        let mut all_types = HashSet::new();
        let mut all_recommendations = HashSet::new();

        for (_, result) in &all_results {
            combined_score += result.risk_score;
            all_findings.extend(result.findings.clone());
            all_types.extend(result.detected_types.iter().copied());
            all_recommendations.extend(result.recommendations.iter().cloned());
        }

        // Normalize combined score
        let final_score = if all_results.is_empty() {
            0
        } else {
            (combined_score / all_results.len() as u32).min(100)
        };

        Ok(ComprehensiveFraudReport {
            user_id: input.user_id.clone(),
            overall_risk_level: RiskLevel::from_score(final_score),
            overall_risk_score: final_score,
            detected_fraud_types: all_types.into_iter().collect(),
            findings: all_findings,
            recommendations: all_recommendations.into_iter().collect(),
            detector_results: all_results,
            analysis_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Quick risk assessment (lighter weight)
    pub async fn quick_assess(&self, input: &FraudAnalysisInput) -> RiskLevel {
        match self.analyze(input).await {
            Ok(report) => report.overall_risk_level,
            Err(_) => RiskLevel::Medium, // Default to medium if analysis fails
        }
    }
}

impl Default for FraudAnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive fraud analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveFraudReport {
    /// User being analyzed
    pub user_id: String,
    /// Overall risk level
    pub overall_risk_level: RiskLevel,
    /// Overall risk score (0-100)
    pub overall_risk_score: u32,
    /// All detected fraud types
    pub detected_fraud_types: Vec<FraudType>,
    /// Combined findings from all detectors
    pub findings: Vec<FraudFinding>,
    /// Combined recommendations
    pub recommendations: Vec<String>,
    /// Results from individual detectors
    pub detector_results: Vec<(String, FraudAnalysisResult)>,
    /// When the analysis was performed
    pub analysis_timestamp: String,
}

impl ComprehensiveFraudReport {
    /// Check if immediate action is required
    #[must_use]
    pub fn requires_immediate_action(&self) -> bool {
        self.overall_risk_level == RiskLevel::Critical
            || self
                .detected_fraud_types
                .contains(&FraudType::IdentityFraud)
    }

    /// Get summary for notifications
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Risk: {:?} (score: {}), Types: {:?}",
            self.overall_risk_level, self.overall_risk_score, self.detected_fraud_types
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_input() -> FraudAnalysisInput {
        FraudAnalysisInput {
            user_id: "user123".to_string(),
            account_created: "2024-01-01T00:00:00Z".to_string(),
            email_domain: Some("gmail.com".to_string()),
            ip_addresses: vec!["192.168.1.1".to_string()],
            user_agents: vec!["Mozilla/5.0".to_string()],
            trades: vec![],
            related_accounts: vec![],
            commitments: vec![],
            evidence_urls: vec![],
        }
    }

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(10), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(30), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(60), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(90), RiskLevel::Critical);
    }

    #[test]
    fn test_fraud_type_severity() {
        assert!(FraudType::IdentityFraud.severity() > FraudType::BotActivity.severity());
        assert!(FraudType::SybilAttack.severity() > FraudType::AnomalousTrading.severity());
    }

    #[tokio::test]
    async fn test_sybil_detector_no_findings() {
        let detector = SybilDetector::new();
        let input = create_test_input();

        let result = detector.analyze(&input).await.unwrap();

        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.findings.is_empty());
    }

    #[tokio::test]
    async fn test_sybil_detector_with_related_accounts() {
        let detector = SybilDetector::new();
        let mut input = create_test_input();

        // Add related accounts
        for i in 0..5 {
            input.related_accounts.push(RelatedAccount {
                user_id: format!("related_{i}"),
                relationship: RelationshipType::SameIp,
                similarity: 0.9,
                evidence: vec!["Same IP".to_string()],
            });
        }

        let result = detector.analyze(&input).await.unwrap();

        assert!(result.risk_score > 0);
        assert!(result.detected_types.contains(&FraudType::SybilAttack));
    }
}
