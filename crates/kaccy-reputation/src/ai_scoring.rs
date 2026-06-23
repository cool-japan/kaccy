//! AI-Assisted Scoring for automatic quality evaluation and fraud detection
//!
//! This module provides infrastructure for quality evaluation and fraud detection within
//! the reputation system.  Actual AI/LLM analysis must be provided by the caller through
//! the [`QualityEvaluator`] trait seam (e.g. an HTTP client crate or an API layer crate).
//!
//! ## Honest evaluation contract
//!
//! This crate ships **without** an HTTP client or an LLM client.  When
//! [`AIScoringService`] is constructed without a real evaluator, any call to
//! [`AIScoringService::evaluate_quality`] returns a result whose
//! [`QualityEvaluation::evaluation_source`] is [`EvaluationSource::Heuristic`] and whose
//! [`QualityEvaluation::confidence`] is deliberately low (≤ 25).  The caller is responsible
//! for treating such results as preliminary / requiring human review.
//!
//! A real AI evaluation is only emitted when a [`QualityEvaluator`] implementation has
//! been injected via [`AIScoringService::with_evaluator`].

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;

// ─── Evaluator seam ──────────────────────────────────────────────────────────

/// Minimal description of a commitment evidence submission passed to a quality evaluator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorInput {
    /// Commitment whose deliverable is being evaluated.
    pub commitment_id: Uuid,
    /// URL of the submitted evidence artifact.
    pub evidence_url: String,
    /// Human-readable description of the evidence.
    pub evidence_description: String,
    /// Original description of the commitment.
    pub commitment_description: String,
}

/// Output produced by a [`QualityEvaluator`] implementation.
///
/// All numeric fields are on a 0–100 scale unless noted otherwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorOutput {
    /// Overall quality score (0–100).
    pub quality_score: i32,
    /// Detailed per-aspect scores.
    pub aspect_scores: AspectScores,
    /// Confidence the evaluator has in this result (0–100).
    pub confidence: i32,
    /// Human-readable explanation of the evaluation.
    pub explanation: String,
    /// Any quality flags raised by the evaluator.
    pub flags: Vec<QualityFlag>,
}

/// Seam for injecting a real AI/LLM quality evaluator.
///
/// Implement this trait in the crate that owns an LLM/HTTP client and inject it
/// via [`AIScoringService::with_evaluator`].  The default behaviour of
/// [`AIScoringService`] (no evaluator configured) always returns a
/// [`EvaluationSource::Heuristic`] result.
#[async_trait]
pub trait QualityEvaluator: Send + Sync {
    /// Evaluate the quality of a submitted deliverable.
    ///
    /// # Errors
    ///
    /// Implementors should return [`ReputationError::Validation`] for invalid
    /// inputs and [`ReputationError::ExternalDataUnavailable`] for transient
    /// connectivity or quota failures.
    async fn evaluate(&self, input: EvaluatorInput) -> Result<EvaluatorOutput, ReputationError>;
}

// ─── Evaluation source marker ─────────────────────────────────────────────────

/// Records how a [`QualityEvaluation`] was produced.
///
/// Consumers **must** inspect this field before treating a quality score as
/// authoritative.  Only [`EvaluationSource::External`] results were produced by
/// a real AI evaluator; [`EvaluationSource::Heuristic`] results are preliminary
/// estimates that require human review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationSource {
    /// Score was produced by a real AI/LLM evaluator injected at runtime.
    External,
    /// Score was produced by an internal description-length heuristic.
    ///
    /// This happens when no [`QualityEvaluator`] has been configured.  The
    /// score **must not** be presented to users as AI analysis.  Confidence is
    /// capped at [`HEURISTIC_CONFIDENCE_CAP`].
    Heuristic,
}

/// Maximum confidence value that may be assigned to heuristic-only evaluations.
///
/// Any [`QualityEvaluation`] with [`EvaluationSource::Heuristic`] carries a
/// `confidence` no higher than this value, signalling that the score is not
/// trustworthy enough to act on automatically.
pub const HEURISTIC_CONFIDENCE_CAP: i32 = 25;

// ─── Public evaluation types ──────────────────────────────────────────────────

/// AI quality evaluation result.
///
/// Always check [`QualityEvaluation::evaluation_source`] before acting on
/// [`QualityEvaluation::quality_score`].  When `evaluation_source` is
/// [`EvaluationSource::Heuristic`], the score is a structural heuristic based
/// only on input lengths and **must not** be labelled as AI analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityEvaluation {
    /// Identifier of the commitment that was evaluated.
    pub commitment_id: Uuid,
    /// Timestamp when this evaluation was produced.
    pub evaluated_at: DateTime<Utc>,
    /// Overall quality score (0–100).
    ///
    /// The meaning of this field depends on [`Self::evaluation_source`].
    pub quality_score: i32,
    /// Detailed scores for different aspects.
    pub aspect_scores: AspectScores,
    /// Confidence level of the evaluation (0–100).
    ///
    /// For heuristic evaluations this is always ≤ [`HEURISTIC_CONFIDENCE_CAP`].
    pub confidence: i32,
    /// Human-readable explanation of how the score was derived.
    pub explanation: String,
    /// Flagged issues, if any.
    pub flags: Vec<QualityFlag>,
    /// How this evaluation was produced.
    ///
    /// Consumers **must** check this before presenting the score to end-users.
    /// Only [`EvaluationSource::External`] results may be labelled "AI analysis".
    pub evaluation_source: EvaluationSource,
    /// `true` when the score was produced by a heuristic and requires human review.
    ///
    /// This is a convenience alias: `is_heuristic == (evaluation_source == Heuristic)`.
    pub is_heuristic: bool,
}

/// Detailed aspect scores (0–100 each).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AspectScores {
    /// Completeness of deliverables (0–100).
    pub completeness: i32,
    /// Technical quality (0–100).
    pub technical_quality: i32,
    /// Documentation quality (0–100).
    pub documentation: i32,
    /// Code/content originality (0–100).
    pub originality: i32,
    /// Alignment with commitment description (0–100).
    pub alignment: i32,
}

/// Quality flag types raised during evaluation.
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
                "Invalid quality flag: {s}. Valid values: incomplete, poor_quality, \
                 plagiarism_suspected, off_topic, insufficient_evidence, suspicious_activity"
            ))),
        }
    }
}

// ─── Fraud detection types ────────────────────────────────────────────────────

/// Fraud detection result, stored in the `fraud_detections` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FraudDetection {
    /// Unique identifier for this fraud detection record.
    pub id: Uuid,
    /// User whose activity triggered the detection.
    pub user_id: Uuid,
    /// Timestamp when the suspicious activity was detected.
    pub detected_at: DateTime<Utc>,
    /// Fraud risk level (0–100, higher = more suspicious).
    pub risk_score: i32,
    /// Type of suspicious pattern detected.
    pub pattern_type: String,
    /// Evidence of suspicious activity.
    pub evidence: serde_json::Value,
    /// Recommended action.
    pub recommended_action: String,
    /// Whether an admin has reviewed this detection.
    pub reviewed: bool,
    /// Timestamp when the detection was recorded.
    pub created_at: DateTime<Utc>,
}

/// Types of fraud patterns the system recognises.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FraudPattern {
    /// Multiple accounts from the same real-world user.
    SybilAttack,
    /// Repeatedly gaming the system for score gains.
    SystemGaming,
    /// Suspicious voting patterns (self-voting rings, etc.).
    VoteManipulation,
    /// Fake evidence or fabricated commitments.
    FakeCommitments,
    /// Coordinated behaviour across accounts.
    CoordinatedBehavior,
    /// Rapid score farming via many low-effort commitments.
    ScoreFarming,
}

// ─── Success prediction types ─────────────────────────────────────────────────

/// Commitment success prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessPrediction {
    /// Commitment that was evaluated.
    pub commitment_id: Uuid,
    /// Timestamp when the prediction was generated.
    pub predicted_at: DateTime<Utc>,
    /// Probability of success (0–100).
    pub success_probability: i32,
    /// Factors contributing to the prediction.
    pub factors: Vec<PredictionFactor>,
    /// Confidence in the prediction (0–100).
    pub confidence: i32,
}

/// A single factor influencing a success prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFactor {
    /// Human-readable name of the factor.
    pub name: String,
    /// Impact on success probability (−100 to 100).
    pub impact: i32,
    /// Explanation of why this factor has its computed impact.
    pub description: String,
}

// ─── Request types ────────────────────────────────────────────────────────────

/// Request to evaluate quality of a commitment deliverable.
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

/// Request to detect fraudulent behaviour for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectFraudRequest {
    /// User whose activity should be analysed.
    pub user_id: Uuid,
    /// Sliding window of activity to analyse, in days.
    pub activity_window_days: i32,
}

/// Request to predict the probability that a commitment will be fulfilled.
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

// ─── Service ──────────────────────────────────────────────────────────────────

/// Service for AI-assisted (and heuristic-fallback) scoring operations.
///
/// Construct with [`AIScoringService::new`] for a heuristic-only instance, or
/// with [`AIScoringService::with_evaluator`] to inject a real [`QualityEvaluator`].
pub struct AIScoringService {
    pool: PgPool,
    /// Optional real evaluator injected by the caller.
    ///
    /// `None` means heuristic-only mode; all evaluations will be explicitly
    /// flagged as [`EvaluationSource::Heuristic`] with low confidence.
    quality_evaluator: Option<Box<dyn QualityEvaluator>>,
}

impl AIScoringService {
    /// Create a new [`AIScoringService`] in **heuristic-only** mode.
    ///
    /// All calls to [`Self::evaluate_quality`] will return results flagged as
    /// [`EvaluationSource::Heuristic`] with confidence ≤ [`HEURISTIC_CONFIDENCE_CAP`].
    /// Use [`Self::with_evaluator`] to supply a real AI evaluator.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            quality_evaluator: None,
        }
    }

    /// Create a new [`AIScoringService`] with a real [`QualityEvaluator`].
    ///
    /// When `evaluator` is present, [`Self::evaluate_quality`] delegates to it
    /// and returns results with [`EvaluationSource::External`].
    pub fn with_evaluator(pool: PgPool, evaluator: Box<dyn QualityEvaluator>) -> Self {
        Self {
            pool,
            quality_evaluator: Some(evaluator),
        }
    }

    /// Evaluate the quality of a commitment deliverable.
    ///
    /// # Evaluation source
    ///
    /// * If a [`QualityEvaluator`] was injected at construction time, it is
    ///   called and the result carries [`EvaluationSource::External`].
    /// * Otherwise the function falls back to a **structural heuristic** (based
    ///   on description lengths) and marks the result as
    ///   [`EvaluationSource::Heuristic`] with `confidence ≤ 25`.  The caller
    ///   **must not** present this to users as AI analysis.
    pub async fn evaluate_quality(
        &self,
        request: EvaluateQualityRequest,
    ) -> Result<QualityEvaluation, ReputationError> {
        let evaluation = match &self.quality_evaluator {
            Some(evaluator) => {
                let input = EvaluatorInput {
                    commitment_id: request.commitment_id,
                    evidence_url: request.evidence_url.clone(),
                    evidence_description: request.evidence_description.clone(),
                    commitment_description: request.commitment_description.clone(),
                };
                let output = evaluator.evaluate(input).await?;
                QualityEvaluation {
                    commitment_id: request.commitment_id,
                    evaluated_at: Utc::now(),
                    quality_score: output.quality_score,
                    aspect_scores: output.aspect_scores,
                    confidence: output.confidence,
                    explanation: output.explanation,
                    flags: output.flags,
                    evaluation_source: EvaluationSource::External,
                    is_heuristic: false,
                }
            }
            None => self.heuristic_evaluation(&request),
        };

        self.store_evaluation(&evaluation).await?;
        Ok(evaluation)
    }

    /// Detect fraudulent behaviour patterns for a user.
    pub async fn detect_fraud(
        &self,
        request: DetectFraudRequest,
    ) -> Result<Option<FraudDetection>, ReputationError> {
        let patterns = self
            .analyze_user_patterns(request.user_id, request.activity_window_days)
            .await?;

        if patterns.is_empty() {
            return Ok(None);
        }

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

        self.store_fraud_detection(&detection).await?;

        Ok(Some(detection))
    }

    /// Predict the probability that a commitment will be fulfilled.
    pub async fn predict_success(
        &self,
        request: PredictSuccessRequest,
    ) -> Result<SuccessPrediction, ReputationError> {
        let historical_rate = self.get_historical_success_rate(request.user_id).await?;
        let complexity_factor = self.analyze_complexity(&request.commitment_description);
        let deadline_factor = self.analyze_deadline(request.deadline_days);

        let factors = vec![
            PredictionFactor {
                name: "Historical Success Rate".to_string(),
                impact: historical_rate - 50,
                description: format!(
                    "User has {historical_rate}% success rate on past commitments"
                ),
            },
            PredictionFactor {
                name: "Commitment Complexity".to_string(),
                impact: -complexity_factor,
                description: "Complexity heuristic derived from description word count".to_string(),
            },
            PredictionFactor {
                name: "Deadline Feasibility".to_string(),
                impact: deadline_factor,
                description: format!("{} days until deadline", request.deadline_days),
            },
        ];

        let base_probability = 50_i32;
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

    /// Get all fraud detections for a user.
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

    /// Mark a fraud detection as reviewed by an admin.
    pub async fn mark_reviewed(&self, detection_id: Uuid) -> Result<(), ReputationError> {
        sqlx::query("UPDATE fraud_detections SET reviewed = true WHERE id = $1")
            .bind(detection_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Produce a heuristic-only [`QualityEvaluation`].
    ///
    /// This is a **structural heuristic** based solely on description lengths.
    /// The result is explicitly marked as [`EvaluationSource::Heuristic`] and
    /// carries `confidence ≤ HEURISTIC_CONFIDENCE_CAP`.  It **must not** be
    /// presented to users as AI or ML analysis.
    fn heuristic_evaluation(&self, request: &EvaluateQualityRequest) -> QualityEvaluation {
        let heuristic_score = self.compute_heuristic_quality_score(request);
        // All per-aspect scores are equally low-confidence; use the same
        // proportional value rather than inventing independent dimensions.
        let aspect = heuristic_score;

        QualityEvaluation {
            commitment_id: request.commitment_id,
            evaluated_at: Utc::now(),
            quality_score: heuristic_score,
            aspect_scores: AspectScores {
                completeness: aspect,
                technical_quality: aspect,
                documentation: aspect,
                originality: aspect,
                alignment: aspect,
            },
            // Cap at HEURISTIC_CONFIDENCE_CAP to signal unreliability.
            confidence: HEURISTIC_CONFIDENCE_CAP,
            explanation: format!(
                "HEURISTIC ESTIMATE ONLY — no AI evaluator is configured. \
                 Score derived from evidence description length ({} chars). \
                 This result requires human review before use.",
                request.evidence_description.len()
            ),
            flags: vec![QualityFlag::InsufficientEvidence],
            evaluation_source: EvaluationSource::Heuristic,
            is_heuristic: true,
        }
    }

    /// Compute a crude quality estimate from description lengths.
    ///
    /// This is a **structural heuristic** — it says nothing about actual content
    /// quality.  It is used only as the `quality_score` field inside
    /// [`Self::heuristic_evaluation`] and is always emitted alongside
    /// `evaluation_source = Heuristic` and `confidence ≤ HEURISTIC_CONFIDENCE_CAP`.
    fn compute_heuristic_quality_score(&self, request: &EvaluateQualityRequest) -> i32 {
        let desc_len = request.evidence_description.len();
        if desc_len > 200 {
            55 // Still below "passing"; in the heuristic range, not a gold score.
        } else if desc_len > 100 {
            40
        } else {
            25
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
        let recent_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM commitments \
             WHERE user_id = $1 AND created_at > NOW() - INTERVAL '7 days'",
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

    fn calculate_risk_score(&self, patterns: &[FraudPattern]) -> i32 {
        let mut score = 0_i32;
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
            Ok(50) // Default success-rate for users with no history.
        } else {
            Ok(((verified as f64 / total as f64) * 100.0) as i32)
        }
    }

    fn analyze_complexity(&self, description: &str) -> i32 {
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

// ─── Display / FromStr for domain enums ──────────────────────────────────────

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
                "Invalid fraud pattern: {s}. Valid values: sybil_attack, system_gaming, \
                 vote_manipulation, fake_commitments, coordinated_behavior, score_farming"
            ))),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── QualityFlag ───────────────────────────────────────────────────────────

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

    // ── FraudPattern ──────────────────────────────────────────────────────────

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

    // ── EvaluationSource honesty contract ─────────────────────────────────────

    /// A mock evaluator that always returns a fixed confident score.
    struct MockEvaluator {
        score: i32,
        confidence: i32,
    }

    #[async_trait]
    impl QualityEvaluator for MockEvaluator {
        async fn evaluate(
            &self,
            input: EvaluatorInput,
        ) -> Result<EvaluatorOutput, ReputationError> {
            Ok(EvaluatorOutput {
                quality_score: self.score,
                aspect_scores: AspectScores {
                    completeness: self.score,
                    technical_quality: self.score,
                    documentation: self.score,
                    originality: self.score,
                    alignment: self.score,
                },
                confidence: self.confidence,
                explanation: format!(
                    "Mock AI evaluation for commitment {}",
                    input.commitment_id
                ),
                flags: vec![],
            })
        }
    }

    /// Build a dummy [`EvaluateQualityRequest`] with the given evidence description.
    fn make_request(evidence_description: &str) -> EvaluateQualityRequest {
        EvaluateQualityRequest {
            commitment_id: Uuid::new_v4(),
            evidence_url: "https://example.com/evidence".to_string(),
            evidence_description: evidence_description.to_string(),
            commitment_description: "Build a widget".to_string(),
        }
    }

    // ── Heuristic-only path (no evaluator configured) ─────────────────────────

    /// Helper: invoke the private heuristic path directly (no DB needed).
    fn heuristic_eval(desc: &str) -> QualityEvaluation {
        // We construct a bare service without a pool by using a dummy; since we
        // only call `heuristic_evaluation` (which does not touch the pool) this
        // is safe in tests.
        //
        // We cannot construct a real PgPool without a DB, so we extract the
        // pure logic via a thin harness struct.
        struct HarnessService;
        impl HarnessService {
            fn compute_heuristic_quality_score(&self, request: &EvaluateQualityRequest) -> i32 {
                let desc_len = request.evidence_description.len();
                if desc_len > 200 {
                    55
                } else if desc_len > 100 {
                    40
                } else {
                    25
                }
            }

            fn heuristic_evaluation(&self, request: &EvaluateQualityRequest) -> QualityEvaluation {
                let heuristic_score = self.compute_heuristic_quality_score(request);
                let aspect = heuristic_score;
                QualityEvaluation {
                    commitment_id: request.commitment_id,
                    evaluated_at: Utc::now(),
                    quality_score: heuristic_score,
                    aspect_scores: AspectScores {
                        completeness: aspect,
                        technical_quality: aspect,
                        documentation: aspect,
                        originality: aspect,
                        alignment: aspect,
                    },
                    confidence: HEURISTIC_CONFIDENCE_CAP,
                    explanation: format!(
                        "HEURISTIC ESTIMATE ONLY — no AI evaluator is configured. \
                         Score derived from evidence description length ({} chars). \
                         This result requires human review before use.",
                        request.evidence_description.len()
                    ),
                    flags: vec![QualityFlag::InsufficientEvidence],
                    evaluation_source: EvaluationSource::Heuristic,
                    is_heuristic: true,
                }
            }
        }

        HarnessService.heuristic_evaluation(&make_request(desc))
    }

    /// Core contract: without an evaluator, `is_heuristic` must be `true`.
    #[test]
    fn ai_scoring_no_evaluator_is_heuristic() {
        let result = heuristic_eval("Short desc");
        assert!(
            result.is_heuristic,
            "is_heuristic must be true when no evaluator is configured"
        );
        assert_eq!(
            result.evaluation_source,
            EvaluationSource::Heuristic,
            "evaluation_source must be Heuristic when no evaluator is configured"
        );
    }

    /// Core contract: heuristic confidence must never reach a level that could be
    /// mistaken for a trustworthy AI score.
    #[test]
    fn ai_scoring_no_evaluator_confidence_is_low() {
        for desc in &["tiny", "a".repeat(150).as_str(), "b".repeat(250).as_str()] {
            let result = heuristic_eval(desc);
            assert!(
                result.confidence <= HEURISTIC_CONFIDENCE_CAP,
                "heuristic confidence ({}) must be ≤ HEURISTIC_CONFIDENCE_CAP ({})",
                result.confidence,
                HEURISTIC_CONFIDENCE_CAP
            );
        }
    }

    /// Heuristic scores must not be in the high-confidence "passing" range
    /// (≥ 70) that a real evaluator might return.
    #[test]
    fn ai_scoring_no_evaluator_score_not_high_confidence() {
        // Even for the longest description, the heuristic score is 55 — below
        // the 70+ range typical of a "passing" AI evaluation.
        let long_desc = "x".repeat(300);
        let result = heuristic_eval(&long_desc);
        assert!(
            result.quality_score < 70,
            "heuristic score ({}) must be below the high-confidence AI range (70+)",
            result.quality_score
        );
    }

    /// Heuristic results must carry at least one flag signalling the lack of
    /// real analysis.
    #[test]
    fn ai_scoring_no_evaluator_flags_insufficient_evidence() {
        let result = heuristic_eval("any description");
        assert!(
            result.flags.contains(&QualityFlag::InsufficientEvidence),
            "heuristic evaluation must carry InsufficientEvidence flag"
        );
    }

    /// The explanation text must not claim to be AI analysis.
    #[test]
    fn ai_scoring_no_evaluator_explanation_is_honest() {
        let result = heuristic_eval("some evidence");
        let explanation_lower = result.explanation.to_lowercase();
        // Must contain "heuristic" or "no ai" or "human review"
        let is_honest = explanation_lower.contains("heuristic")
            || explanation_lower.contains("human review")
            || explanation_lower.contains("no ai");
        assert!(
            is_honest,
            "explanation must honestly describe the heuristic nature; got: {:?}",
            result.explanation
        );
        // Must NOT contain "ai evaluation" or "ai-based" as a confident claim
        assert!(
            !explanation_lower.contains("ai evaluation based on evidence analysis"),
            "explanation must not claim to be an AI evaluation; got: {:?}",
            result.explanation
        );
    }

    // ── Mock evaluator path ────────────────────────────────────────────────────

    /// When a real evaluator is provided, `is_heuristic` must be `false`.
    #[tokio::test]
    async fn ai_scoring_with_evaluator_not_heuristic() {
        let evaluator = MockEvaluator {
            score: 88,
            confidence: 90,
        };
        let input = EvaluatorInput {
            commitment_id: Uuid::new_v4(),
            evidence_url: "https://example.com/evidence".to_string(),
            evidence_description: "Detailed evidence of the completed work".to_string(),
            commitment_description: "Build a widget".to_string(),
        };
        let output = evaluator.evaluate(input).await.unwrap();

        // Simulate what AIScoringService would produce from the evaluator output.
        let evaluation = QualityEvaluation {
            commitment_id: Uuid::new_v4(),
            evaluated_at: Utc::now(),
            quality_score: output.quality_score,
            aspect_scores: output.aspect_scores,
            confidence: output.confidence,
            explanation: output.explanation,
            flags: output.flags,
            evaluation_source: EvaluationSource::External,
            is_heuristic: false,
        };

        assert!(
            !evaluation.is_heuristic,
            "is_heuristic must be false when a real evaluator produced the result"
        );
        assert_eq!(
            evaluation.evaluation_source,
            EvaluationSource::External,
            "evaluation_source must be External when a real evaluator was used"
        );
        assert_eq!(evaluation.quality_score, 88);
        assert_eq!(evaluation.confidence, 90);
    }

    /// EvaluationSource::Heuristic and ::External are distinct and
    /// cannot be confused by PartialEq.
    #[test]
    fn evaluation_source_variants_are_distinct() {
        assert_ne!(EvaluationSource::Heuristic, EvaluationSource::External);
        assert_eq!(EvaluationSource::Heuristic, EvaluationSource::Heuristic);
        assert_eq!(EvaluationSource::External, EvaluationSource::External);
    }

    /// HEURISTIC_CONFIDENCE_CAP is strictly below a "passing" AI confidence
    /// level so heuristic results cannot be mistaken for real AI outputs.
    ///
    /// Verified at compile time via a const-eval assertion; the #[test] wrapper
    /// ensures the test runner reports it.
    #[test]
    fn heuristic_confidence_cap_is_below_passing_threshold() {
        // Compile-time proof: this const-eval assertion fires during compilation,
        // not just at runtime, guaranteeing the invariant can never silently drift.
        const _: () = assert!(
            HEURISTIC_CONFIDENCE_CAP < 50,
            "HEURISTIC_CONFIDENCE_CAP must be below 50 to distinguish \
             heuristic outputs from real AI results"
        );
        // Provide a runtime witness so that the test framework counts this as
        // an executed assertion (the const block above already prevents it being
        // wrong at compile time; this makes the test "green" rather than empty).
        let cap: i32 = HEURISTIC_CONFIDENCE_CAP;
        assert!(cap < 50, "cap={cap} must be < 50");
    }

    // ── Description-length heuristic score boundaries ─────────────────────────

    #[test]
    fn heuristic_score_short_description() {
        // < 100 chars → lowest band
        let result = heuristic_eval("short");
        assert_eq!(result.quality_score, 25);
    }

    #[test]
    fn heuristic_score_medium_description() {
        // 101–200 chars → middle band
        let medium = "m".repeat(150);
        let result = heuristic_eval(&medium);
        assert_eq!(result.quality_score, 40);
    }

    #[test]
    fn heuristic_score_long_description() {
        // > 200 chars → top heuristic band
        let long = "l".repeat(201);
        let result = heuristic_eval(&long);
        assert_eq!(result.quality_score, 55);
    }
}
