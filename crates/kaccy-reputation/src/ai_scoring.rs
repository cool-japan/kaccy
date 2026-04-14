//! AI-Assisted Scoring for automatic quality evaluation and fraud detection
//!
//! This module provides AI-powered analysis of commitments and user behavior
//! to assist in reputation scoring and fraud prevention.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;

/// AI quality evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityEvaluation {
    /// Identifier of the commitment that was evaluated.
    pub commitment_id: Uuid,
    /// Timestamp when this evaluation was produced.
    pub evaluated_at: DateTime<Utc>,
    /// Overall quality score (0-100)
    pub quality_score: i32,
    /// Detailed scores for different aspects
    pub aspect_scores: AspectScores,
    /// Confidence level of the evaluation (0-100)
    pub confidence: i32,
    /// AI-generated explanation
    pub explanation: String,
    /// Flagged issues if any
    pub flags: Vec<QualityFlag>,
}

/// Detailed aspect scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AspectScores {
    /// Completeness of deliverables (0-100)
    pub completeness: i32,
    /// Technical quality (0-100)
    pub technical_quality: i32,
    /// Documentation quality (0-100)
    pub documentation: i32,
    /// Code/content originality (0-100)
    pub originality: i32,
    /// Alignment with commitment description (0-100)
    pub alignment: i32,
}

/// Quality flag types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum QualityFlag {
    /// Deliverable is missing required parts.
    Incomplete,
    /// Work does not meet minimum quality standards.
    PoorQuality,
    /// Content appears to have been copied from another source.
    PlagiarismSuspected,
    /// Submitted work does not match the commitment description.
    OffTopic,
    /// Supporting evidence is missing or inadequate.
    InsufficientEvidence,
    /// Behavioral patterns suggest fraudulent activity.
    SuspiciousActivity,
}

impl std::fmt::Display for QualityFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete => write!(f, "Incomplete"),
            Self::PoorQuality => write!(f, "Poor Quality"),
            Self::PlagiarismSuspected => write!(f, "Plagiarism Suspected"),
            Self::OffTopic => write!(f, "Off Topic"),
            Self::InsufficientEvidence => write!(f, "Insufficient Evidence"),
            Self::SuspiciousActivity => write!(f, "Suspicious Activity"),
        }
    }
}

impl std::str::FromStr for QualityFlag {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace([' ', '-'], "_").as_str() {
            "incomplete" => Ok(QualityFlag::Incomplete),
            "poor_quality" | "poorquality" => Ok(QualityFlag::PoorQuality),
            "plagiarism_suspected" | "plagiarismsuspected" => Ok(QualityFlag::PlagiarismSuspected),
            "off_topic" | "offtopic" => Ok(QualityFlag::OffTopic),
            "insufficient_evidence" | "insufficientevidence" => {
                Ok(QualityFlag::InsufficientEvidence)
            }
            "suspicious_activity" | "suspiciousactivity" => Ok(QualityFlag::SuspiciousActivity),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid quality flag: {}. Valid values: incomplete, poor_quality, plagiarism_suspected, off_topic, insufficient_evidence, suspicious_activity",
                s
            ))),
        }
    }
}

/// Fraud detection result
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FraudDetection {
    /// Unique identifier for this fraud detection record.
    pub id: Uuid,
    /// User whose activity triggered the detection.
    pub user_id: Uuid,
    /// Timestamp when the suspicious activity was detected.
    pub detected_at: DateTime<Utc>,
    /// Fraud risk level (0-100, higher = more suspicious)
    pub risk_score: i32,
    /// Type of suspicious pattern detected
    pub pattern_type: String,
    /// Evidence of suspicious activity
    pub evidence: serde_json::Value,
    /// Recommended action
    pub recommended_action: String,
    /// Whether admin has reviewed this
    pub reviewed: bool,
    /// Timestamp when the detection was recorded.
    pub created_at: DateTime<Utc>,
}

/// Types of fraud patterns
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FraudPattern {
    /// Multiple accounts from same user
    SybilAttack,
    /// Repeatedly gaming the system
    SystemGaming,
    /// Suspicious voting patterns
    VoteManipulation,
    /// Fake evidence or commitments
    FakeCommitments,
    /// Coordinated behavior across accounts
    CoordinatedBehavior,
    /// Rapid score farming
    ScoreFarming,
}

/// Commitment success prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessPrediction {
    /// Commitment that was evaluated.
    pub commitment_id: Uuid,
    /// Timestamp when the prediction was generated.
    pub predicted_at: DateTime<Utc>,
    /// Probability of success (0-100)
    pub success_probability: i32,
    /// Factors contributing to prediction
    pub factors: Vec<PredictionFactor>,
    /// Confidence in the prediction (0-100)
    pub confidence: i32,
}

/// Factor influencing success prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFactor {
    /// Human-readable name of the factor.
    pub name: String,
    /// Impact on success probability (-100 to 100)
    pub impact: i32,
    /// Explanation of why this factor has its computed impact.
    pub description: String,
}

/// Request to evaluate quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateQualityRequest {
    /// Commitment whose deliverable is being evaluated.
    pub commitment_id: Uuid,
    /// URL of the submitted evidence artifact.
    pub evidence_url: String,
    /// Human-readable description of the evidence.
    pub evidence_description: String,
    /// Original description of the commitment.
    pub commitment_description: String,
}

/// Request to detect fraud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectFraudRequest {
    /// User whose activity should be analysed.
    pub user_id: Uuid,
    /// Recent activity to analyze
    pub activity_window_days: i32,
}

/// Request to predict success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictSuccessRequest {
    /// Commitment to evaluate.
    pub commitment_id: Uuid,
    /// User who owns the commitment.
    pub user_id: Uuid,
    /// Description of the commitment's deliverable.
    pub commitment_description: String,
    /// Number of days until the commitment deadline.
    pub deadline_days: i32,
}

/// Service for AI-assisted scoring operations
pub struct AIScoringService {
    pool: PgPool,
}

impl AIScoringService {
    /// Create a new [`AIScoringService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Evaluate the quality of a commitment deliverable
    pub async fn evaluate_quality(
        &self,
        request: EvaluateQualityRequest,
    ) -> Result<QualityEvaluation, ReputationError> {
        // In a real implementation, this would call an AI service
        // For now, we'll return a placeholder result

        let evaluation = QualityEvaluation {
            commitment_id: request.commitment_id,
            evaluated_at: Utc::now(),
            quality_score: self.calculate_placeholder_quality(&request),
            aspect_scores: AspectScores {
                completeness: 80,
                technical_quality: 75,
                documentation: 70,
                originality: 85,
                alignment: 90,
            },
            confidence: 75,
            explanation: "AI evaluation based on evidence analysis.".to_string(),
            flags: vec![],
        };

        // Store evaluation in database
        self.store_evaluation(&evaluation).await?;

        Ok(evaluation)
    }

    /// Detect fraudulent behavior patterns
    pub async fn detect_fraud(
        &self,
        request: DetectFraudRequest,
    ) -> Result<Option<FraudDetection>, ReputationError> {
        // Analyze user activity for suspicious patterns
        let patterns = self
            .analyze_user_patterns(request.user_id, request.activity_window_days)
            .await?;

        if patterns.is_empty() {
            return Ok(None);
        }

        // Calculate risk score
        let risk_score = self.calculate_risk_score(&patterns);

        if risk_score < 50 {
            return Ok(None);
        }

        let detection = FraudDetection {
            id: Uuid::new_v4(),
            user_id: request.user_id,
            detected_at: Utc::now(),
            risk_score,
            pattern_type: patterns[0].to_string(),
            evidence: serde_json::json!({"patterns": patterns}),
            recommended_action: self.recommend_action(risk_score),
            reviewed: false,
            created_at: Utc::now(),
        };

        // Store fraud detection
        self.store_fraud_detection(&detection).await?;

        Ok(Some(detection))
    }

    /// Predict the probability of commitment success
    pub async fn predict_success(
        &self,
        request: PredictSuccessRequest,
    ) -> Result<SuccessPrediction, ReputationError> {
        // Get user's historical success rate
        let historical_rate = self.get_historical_success_rate(request.user_id).await?;

        // Analyze commitment complexity
        let complexity_factor = self.analyze_complexity(&request.commitment_description);

        // Analyze deadline feasibility
        let deadline_factor = self.analyze_deadline(request.deadline_days);

        let factors = vec![
            PredictionFactor {
                name: "Historical Success Rate".to_string(),
                impact: historical_rate - 50,
                description: format!(
                    "User has {}% success rate on past commitments",
                    historical_rate
                ),
            },
            PredictionFactor {
                name: "Commitment Complexity".to_string(),
                impact: -complexity_factor,
                description: "Complexity analysis of the commitment".to_string(),
            },
            PredictionFactor {
                name: "Deadline Feasibility".to_string(),
                impact: deadline_factor,
                description: format!("{} days until deadline", request.deadline_days),
            },
        ];

        let base_probability = 50;
        let total_impact: i32 = factors.iter().map(|f| f.impact).sum();
        let success_probability = (base_probability + total_impact).clamp(0, 100);

        Ok(SuccessPrediction {
            commitment_id: request.commitment_id,
            predicted_at: Utc::now(),
            success_probability,
            factors,
            confidence: 70,
        })
    }

    /// Get fraud detections for a user
    pub async fn get_fraud_detections(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<FraudDetection>, ReputationError> {
        let detections = sqlx::query_as::<_, FraudDetection>(
            "SELECT * FROM fraud_detections WHERE user_id = $1 ORDER BY detected_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(detections)
    }

    /// Mark a fraud detection as reviewed
    pub async fn mark_reviewed(&self, detection_id: Uuid) -> Result<(), ReputationError> {
        sqlx::query("UPDATE fraud_detections SET reviewed = true WHERE id = $1")
            .bind(detection_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Helper methods

    #[allow(dead_code)]
    fn calculate_placeholder_quality(&self, request: &EvaluateQualityRequest) -> i32 {
        // Placeholder logic based on description length
        let desc_len = request.evidence_description.len();
        if desc_len > 200 {
            85
        } else if desc_len > 100 {
            70
        } else {
            60
        }
    }

    async fn store_evaluation(
        &self,
        evaluation: &QualityEvaluation,
    ) -> Result<(), ReputationError> {
        sqlx::query(
            r#"
            INSERT INTO ai_quality_evaluations (
                id, commitment_id, quality_score, confidence, explanation, evaluated_at, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(evaluation.commitment_id)
        .bind(evaluation.quality_score)
        .bind(evaluation.confidence)
        .bind(&evaluation.explanation)
        .bind(evaluation.evaluated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn analyze_user_patterns(
        &self,
        user_id: Uuid,
        _window_days: i32,
    ) -> Result<Vec<FraudPattern>, ReputationError> {
        // Check for rapid commitment creation
        let recent_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM commitments WHERE user_id = $1 AND created_at > NOW() - INTERVAL '7 days'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let mut patterns = Vec::new();

        if recent_count > 10 {
            patterns.push(FraudPattern::ScoreFarming);
        }

        Ok(patterns)
    }

    #[allow(dead_code)]
    fn calculate_risk_score(&self, patterns: &[FraudPattern]) -> i32 {
        let mut score = 0;
        for pattern in patterns {
            score += match pattern {
                FraudPattern::SybilAttack => 80,
                FraudPattern::SystemGaming => 60,
                FraudPattern::VoteManipulation => 70,
                FraudPattern::FakeCommitments => 90,
                FraudPattern::CoordinatedBehavior => 75,
                FraudPattern::ScoreFarming => 50,
            };
        }
        score.min(100)
    }

    fn recommend_action(&self, risk_score: i32) -> String {
        if risk_score >= 80 {
            "Suspend account pending review".to_string()
        } else if risk_score >= 60 {
            "Flag for manual review".to_string()
        } else {
            "Monitor activity".to_string()
        }
    }

    async fn store_fraud_detection(
        &self,
        detection: &FraudDetection,
    ) -> Result<(), ReputationError> {
        sqlx::query(
            r#"
            INSERT INTO fraud_detections (
                id, user_id, detected_at, risk_score, pattern_type, evidence,
                recommended_action, reviewed, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(detection.id)
        .bind(detection.user_id)
        .bind(detection.detected_at)
        .bind(detection.risk_score)
        .bind(&detection.pattern_type)
        .bind(&detection.evidence)
        .bind(&detection.recommended_action)
        .bind(detection.reviewed)
        .bind(detection.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_historical_success_rate(&self, user_id: Uuid) -> Result<i32, ReputationError> {
        let result = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'verified') as verified,
                COUNT(*) as total
            FROM commitments
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let (verified, total) = result;
        if total == 0 {
            Ok(50) // Default for new users
        } else {
            Ok(((verified as f64 / total as f64) * 100.0) as i32)
        }
    }

    fn analyze_complexity(&self, description: &str) -> i32 {
        // Simple heuristic: longer descriptions = more complex
        let words = description.split_whitespace().count();
        if words > 100 {
            20
        } else if words > 50 {
            10
        } else {
            0
        }
    }

    fn analyze_deadline(&self, days: i32) -> i32 {
        // More time = better chance of success
        if days > 60 {
            20
        } else if days > 30 {
            10
        } else if days > 14 {
            0
        } else {
            -10
        }
    }
}

impl std::fmt::Display for FraudPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FraudPattern::SybilAttack => write!(f, "sybil_attack"),
            FraudPattern::SystemGaming => write!(f, "system_gaming"),
            FraudPattern::VoteManipulation => write!(f, "vote_manipulation"),
            FraudPattern::FakeCommitments => write!(f, "fake_commitments"),
            FraudPattern::CoordinatedBehavior => write!(f, "coordinated_behavior"),
            FraudPattern::ScoreFarming => write!(f, "score_farming"),
        }
    }
}

impl std::str::FromStr for FraudPattern {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace([' ', '-'], "_").as_str() {
            "sybil_attack" | "sybilattack" => Ok(FraudPattern::SybilAttack),
            "system_gaming" | "systemgaming" => Ok(FraudPattern::SystemGaming),
            "vote_manipulation" | "votemanipulation" => Ok(FraudPattern::VoteManipulation),
            "fake_commitments" | "fakecommitments" => Ok(FraudPattern::FakeCommitments),
            "coordinated_behavior" | "coordinatedbehavior" => Ok(FraudPattern::CoordinatedBehavior),
            "score_farming" | "scorefarming" => Ok(FraudPattern::ScoreFarming),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid fraud pattern: {}. Valid values: sybil_attack, system_gaming, vote_manipulation, fake_commitments, coordinated_behavior, score_farming",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_flag_display() {
        assert_eq!(QualityFlag::Incomplete.to_string(), "Incomplete");
        assert_eq!(QualityFlag::PoorQuality.to_string(), "Poor Quality");
        assert_eq!(
            QualityFlag::PlagiarismSuspected.to_string(),
            "Plagiarism Suspected"
        );
        assert_eq!(QualityFlag::OffTopic.to_string(), "Off Topic");
        assert_eq!(
            QualityFlag::InsufficientEvidence.to_string(),
            "Insufficient Evidence"
        );
        assert_eq!(
            QualityFlag::SuspiciousActivity.to_string(),
            "Suspicious Activity"
        );
    }

    #[test]
    fn test_quality_flag_from_str_valid() {
        assert_eq!(
            "incomplete".parse::<QualityFlag>().unwrap(),
            QualityFlag::Incomplete
        );
        assert_eq!(
            "poor_quality".parse::<QualityFlag>().unwrap(),
            QualityFlag::PoorQuality
        );
        assert_eq!(
            "plagiarism_suspected".parse::<QualityFlag>().unwrap(),
            QualityFlag::PlagiarismSuspected
        );
        assert_eq!(
            "off_topic".parse::<QualityFlag>().unwrap(),
            QualityFlag::OffTopic
        );
        assert_eq!(
            "insufficient_evidence".parse::<QualityFlag>().unwrap(),
            QualityFlag::InsufficientEvidence
        );
        assert_eq!(
            "suspicious_activity".parse::<QualityFlag>().unwrap(),
            QualityFlag::SuspiciousActivity
        );
    }

    #[test]
    fn test_quality_flag_from_str_variants() {
        assert_eq!(
            "poor quality".parse::<QualityFlag>().unwrap(),
            QualityFlag::PoorQuality
        );
        assert_eq!(
            "poorquality".parse::<QualityFlag>().unwrap(),
            QualityFlag::PoorQuality
        );
        assert_eq!(
            "off topic".parse::<QualityFlag>().unwrap(),
            QualityFlag::OffTopic
        );
        assert_eq!(
            "offtopic".parse::<QualityFlag>().unwrap(),
            QualityFlag::OffTopic
        );
    }

    #[test]
    fn test_quality_flag_from_str_case_insensitive() {
        assert_eq!(
            "INCOMPLETE".parse::<QualityFlag>().unwrap(),
            QualityFlag::Incomplete
        );
        assert_eq!(
            "Poor_Quality".parse::<QualityFlag>().unwrap(),
            QualityFlag::PoorQuality
        );
    }

    #[test]
    fn test_quality_flag_from_str_invalid() {
        assert!("invalid".parse::<QualityFlag>().is_err());
        assert!("unknown".parse::<QualityFlag>().is_err());
        assert!("".parse::<QualityFlag>().is_err());
    }

    #[test]
    fn test_fraud_pattern_from_str_valid() {
        assert_eq!(
            "sybil_attack".parse::<FraudPattern>().unwrap(),
            FraudPattern::SybilAttack
        );
        assert_eq!(
            "system_gaming".parse::<FraudPattern>().unwrap(),
            FraudPattern::SystemGaming
        );
        assert_eq!(
            "vote_manipulation".parse::<FraudPattern>().unwrap(),
            FraudPattern::VoteManipulation
        );
        assert_eq!(
            "fake_commitments".parse::<FraudPattern>().unwrap(),
            FraudPattern::FakeCommitments
        );
        assert_eq!(
            "coordinated_behavior".parse::<FraudPattern>().unwrap(),
            FraudPattern::CoordinatedBehavior
        );
        assert_eq!(
            "score_farming".parse::<FraudPattern>().unwrap(),
            FraudPattern::ScoreFarming
        );
    }

    #[test]
    fn test_fraud_pattern_from_str_variants() {
        assert_eq!(
            "sybil attack".parse::<FraudPattern>().unwrap(),
            FraudPattern::SybilAttack
        );
        assert_eq!(
            "sybilattack".parse::<FraudPattern>().unwrap(),
            FraudPattern::SybilAttack
        );
        assert_eq!(
            "system gaming".parse::<FraudPattern>().unwrap(),
            FraudPattern::SystemGaming
        );
    }

    #[test]
    fn test_fraud_pattern_from_str_case_insensitive() {
        assert_eq!(
            "SYBIL_ATTACK".parse::<FraudPattern>().unwrap(),
            FraudPattern::SybilAttack
        );
        assert_eq!(
            "System_Gaming".parse::<FraudPattern>().unwrap(),
            FraudPattern::SystemGaming
        );
    }

    #[test]
    fn test_fraud_pattern_from_str_invalid() {
        assert!("invalid".parse::<FraudPattern>().is_err());
        assert!("unknown".parse::<FraudPattern>().is_err());
        assert!("".parse::<FraudPattern>().is_err());
    }
}
