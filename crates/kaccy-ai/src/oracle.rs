//! AI Oracle for fully automated verification
//!
//! This module provides advanced AI-powered verification that:
//! - Uses multi-model consensus for high-confidence decisions
//! - Learns from admin feedback over time
//! - Automatically verifies commitments without human intervention

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;

use crate::error::{AiError, Result};
use crate::{AiCommitmentVerifier, VerificationRequest};

/// Consensus strategy for multi-model verification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusStrategy {
    /// Unanimous agreement required
    Unanimous,
    /// Majority vote (> 50%)
    Majority,
    /// Weighted average based on model reliability
    WeightedAverage,
}

/// Model weight for consensus calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWeight {
    /// Model identifier
    pub model: String,
    /// Weight (0.0 - 1.0), higher means more trusted
    pub weight: f64,
    /// Historical accuracy for this model
    pub accuracy: f64,
}

impl ModelWeight {
    /// Create a new model weight with default accuracy
    pub fn new(model: impl Into<String>, weight: f64) -> Self {
        Self {
            model: model.into(),
            weight: weight.clamp(0.0, 1.0),
            accuracy: 0.5,
        }
    }

    /// Update accuracy based on feedback
    pub fn update_accuracy(&mut self, correct: bool) {
        // Exponential moving average
        let alpha = 0.1;
        let new_value = if correct { 1.0 } else { 0.0 };
        self.accuracy = alpha * new_value + (1.0 - alpha) * self.accuracy;

        // Adjust weight based on accuracy
        self.weight = self.accuracy;
    }
}

/// Multi-model consensus result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// Final decision
    pub approved: bool,
    /// Consensus confidence (0-100)
    pub confidence: f64,
    /// Individual model votes
    pub votes: Vec<ModelVote>,
    /// Strategy used
    pub strategy: ConsensusStrategy,
    /// Final reasoning
    pub reasoning: String,
}

/// Individual model vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVote {
    /// Model identifier
    pub model: String,
    /// Approved or rejected
    pub approved: bool,
    /// Confidence score (0-100)
    pub confidence: f64,
    /// Reasoning
    pub reasoning: String,
}

/// Configuration for the AI Oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleConfig {
    /// Consensus strategy
    pub strategy: ConsensusStrategy,
    /// Model weights for consensus
    pub model_weights: Vec<ModelWeight>,
    /// Minimum confidence threshold for automated approval (0-100)
    pub min_confidence: f64,
    /// Maximum confidence threshold for automated rejection (0-100)
    pub max_rejection_confidence: f64,
    /// Enable learning from feedback
    pub enable_learning: bool,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self {
            strategy: ConsensusStrategy::WeightedAverage,
            model_weights: vec![
                ModelWeight::new("claude-3-opus-20240229", 0.9),
                ModelWeight::new("claude-3-5-sonnet-20241022", 0.85),
                ModelWeight::new("gpt-4-turbo", 0.8),
            ],
            min_confidence: 85.0,
            max_rejection_confidence: 30.0,
            enable_learning: true,
        }
    }
}

/// AI Oracle for automated verification
pub struct AiOracle {
    config: OracleConfig,
    verifiers: HashMap<String, AiCommitmentVerifier>,
    feedback_log: Vec<FeedbackEntry>,
}

/// Feedback entry for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    /// Verification request
    pub commitment_id: String,
    /// Oracle decision
    pub oracle_decision: bool,
    /// Oracle confidence
    pub oracle_confidence: f64,
    /// Admin override decision
    pub admin_decision: bool,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl AiOracle {
    /// Create a new AI Oracle
    #[must_use]
    pub fn new(config: OracleConfig) -> Self {
        Self {
            config,
            verifiers: HashMap::new(),
            feedback_log: Vec::new(),
        }
    }

    /// Add a verifier for a specific model
    pub fn add_verifier(&mut self, model: String, verifier: AiCommitmentVerifier) {
        self.verifiers.insert(model, verifier);
    }

    /// Verify a commitment with multi-model consensus
    pub async fn verify_with_consensus(
        &self,
        request: &VerificationRequest,
    ) -> Result<ConsensusResult> {
        let mut votes = Vec::new();

        // Get verification from each model
        for weight in &self.config.model_weights {
            if let Some(verifier) = self.verifiers.get(&weight.model) {
                match verifier.verify_evidence(request).await {
                    Ok(result) => {
                        votes.push(ModelVote {
                            model: weight.model.clone(),
                            approved: result.fulfilled,
                            confidence: result.confidence,
                            reasoning: result.reasoning.clone(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            model = %weight.model,
                            error = %e,
                            "Model verification failed"
                        );
                    }
                }
            }
        }

        if votes.is_empty() {
            return Err(AiError::Unavailable(
                "No verifiers available for consensus".to_string(),
            ));
        }

        // Calculate consensus
        let (approved, confidence, reasoning) = self.calculate_consensus(&votes)?;

        Ok(ConsensusResult {
            approved,
            confidence,
            votes,
            strategy: self.config.strategy,
            reasoning,
        })
    }

    /// Calculate consensus from votes
    fn calculate_consensus(&self, votes: &[ModelVote]) -> Result<(bool, f64, String)> {
        match self.config.strategy {
            ConsensusStrategy::Unanimous => self.unanimous_consensus(votes),
            ConsensusStrategy::Majority => self.majority_consensus(votes),
            ConsensusStrategy::WeightedAverage => self.weighted_consensus(votes),
        }
    }

    /// Unanimous consensus (all models must agree)
    fn unanimous_consensus(&self, votes: &[ModelVote]) -> Result<(bool, f64, String)> {
        let all_approved = votes.iter().all(|v| v.approved);
        let all_rejected = votes.iter().all(|v| !v.approved);

        if all_approved {
            let avg_confidence =
                votes.iter().map(|v| v.confidence).sum::<f64>() / votes.len() as f64;
            let reasoning = format!(
                "All {} models unanimously approved with average confidence {:.1}%",
                votes.len(),
                avg_confidence
            );
            Ok((true, avg_confidence, reasoning))
        } else if all_rejected {
            let avg_confidence =
                votes.iter().map(|v| v.confidence).sum::<f64>() / votes.len() as f64;
            let reasoning = format!(
                "All {} models unanimously rejected with average confidence {:.1}%",
                votes.len(),
                avg_confidence
            );
            Ok((false, avg_confidence, reasoning))
        } else {
            let reasoning = format!(
                "No unanimous consensus: {} approved, {} rejected",
                votes.iter().filter(|v| v.approved).count(),
                votes.iter().filter(|v| !v.approved).count()
            );
            Ok((false, 50.0, reasoning))
        }
    }

    /// Majority consensus (> 50% agreement)
    fn majority_consensus(&self, votes: &[ModelVote]) -> Result<(bool, f64, String)> {
        let approved_count = votes.iter().filter(|v| v.approved).count();
        let total_count = votes.len();
        let approved = approved_count > total_count / 2;

        let avg_confidence = votes
            .iter()
            .filter(|v| v.approved == approved)
            .map(|v| v.confidence)
            .sum::<f64>()
            / approved_count.max(1) as f64;

        let reasoning = format!(
            "Majority {} ({}/{}): average confidence {:.1}%",
            if approved { "approved" } else { "rejected" },
            if approved {
                approved_count
            } else {
                total_count - approved_count
            },
            total_count,
            avg_confidence
        );

        Ok((approved, avg_confidence, reasoning))
    }

    /// Weighted consensus (based on model weights and accuracy)
    fn weighted_consensus(&self, votes: &[ModelVote]) -> Result<(bool, f64, String)> {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        let mut weighted_confidence = 0.0;

        for vote in votes {
            if let Some(weight) = self
                .config
                .model_weights
                .iter()
                .find(|w| w.model == vote.model)
            {
                let vote_value = if vote.approved { 1.0 } else { 0.0 };
                weighted_sum += vote_value * weight.weight;
                weighted_confidence += vote.confidence * weight.weight;
                total_weight += weight.weight;
            }
        }

        if total_weight == 0.0 {
            return Err(AiError::Internal("No model weights configured".to_string()));
        }

        let weighted_score = weighted_sum / total_weight;
        let approved = weighted_score > 0.5;
        let confidence = weighted_confidence / total_weight;

        let reasoning = format!(
            "Weighted consensus {} (score: {:.2}, confidence: {:.1}%)",
            if approved { "approved" } else { "rejected" },
            weighted_score,
            confidence
        );

        Ok((approved, confidence, reasoning))
    }

    /// Record admin feedback for learning
    pub fn record_feedback(
        &mut self,
        commitment_id: String,
        oracle_decision: bool,
        oracle_confidence: f64,
        admin_decision: bool,
    ) {
        if !self.config.enable_learning {
            return;
        }

        let feedback = FeedbackEntry {
            commitment_id,
            oracle_decision,
            oracle_confidence,
            admin_decision,
            timestamp: chrono::Utc::now(),
        };

        // Update model weights based on feedback
        let oracle_correct = oracle_decision == admin_decision;

        for weight in &mut self.config.model_weights {
            weight.update_accuracy(oracle_correct);
        }

        self.feedback_log.push(feedback);

        tracing::info!(
            oracle_correct = oracle_correct,
            "Recorded feedback and updated model weights"
        );
    }

    /// Get learning statistics
    #[must_use]
    pub fn get_learning_stats(&self) -> LearningStats {
        let total_feedback = self.feedback_log.len();
        let correct_decisions = self
            .feedback_log
            .iter()
            .filter(|f| f.oracle_decision == f.admin_decision)
            .count();

        let accuracy = if total_feedback > 0 {
            (correct_decisions as f64 / total_feedback as f64) * 100.0
        } else {
            0.0
        };

        LearningStats {
            total_feedback,
            correct_decisions,
            accuracy,
            model_accuracies: self
                .config
                .model_weights
                .iter()
                .map(|w| (w.model.clone(), w.accuracy * 100.0))
                .collect(),
        }
    }

    /// Check if oracle can make automated decision
    #[must_use]
    pub fn can_auto_decide(&self, consensus: &ConsensusResult) -> AutoDecision {
        if consensus.confidence >= self.config.min_confidence && consensus.approved {
            AutoDecision::AutoApprove
        } else if consensus.confidence >= self.config.min_confidence && !consensus.approved {
            AutoDecision::AutoReject
        } else {
            AutoDecision::NeedsHumanReview
        }
    }

    /// Save oracle state to file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kaccy_ai::{AiOracle, OracleConfig};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let oracle = AiOracle::new(OracleConfig::default());
    /// oracle.save_to_file("oracle_state.json")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let state = OracleState {
            config: self.config.clone(),
            feedback_log: self.feedback_log.clone(),
        };

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| AiError::Internal(format!("Failed to serialize oracle state: {e}")))?;

        std::fs::write(path, json)
            .map_err(|e| AiError::Internal(format!("Failed to write oracle state: {e}")))?;

        Ok(())
    }

    /// Load oracle state from file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kaccy_ai::{AiOracle, OracleConfig};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut oracle = AiOracle::new(OracleConfig::default());
    /// oracle.load_from_file("oracle_state.json")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| AiError::Internal(format!("Failed to read oracle state: {e}")))?;

        let state: OracleState = serde_json::from_str(&json)
            .map_err(|e| AiError::Internal(format!("Failed to deserialize oracle state: {e}")))?;

        self.config = state.config;
        self.feedback_log = state.feedback_log;

        Ok(())
    }

    /// Export feedback log as CSV
    pub fn export_feedback_csv(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let mut csv = String::from(
            "commitment_id,oracle_decision,oracle_confidence,admin_decision,timestamp,correct\n",
        );

        for feedback in &self.feedback_log {
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{}",
                feedback.commitment_id,
                feedback.oracle_decision,
                feedback.oracle_confidence,
                feedback.admin_decision,
                feedback.timestamp.to_rfc3339(),
                feedback.oracle_decision == feedback.admin_decision
            );
        }

        std::fs::write(path, csv)
            .map_err(|e| AiError::Internal(format!("Failed to write feedback CSV: {e}")))?;

        Ok(())
    }

    /// Get feedback log
    #[must_use]
    pub fn get_feedback_log(&self) -> &[FeedbackEntry] {
        &self.feedback_log
    }

    /// Clear feedback log (useful for testing or reset)
    pub fn clear_feedback_log(&mut self) {
        self.feedback_log.clear();
    }

    /// Process batch of feedback entries for learning
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_ai::{AiOracle, OracleConfig, FeedbackEntry};
    ///
    /// let mut oracle = AiOracle::new(OracleConfig::default());
    /// let feedback_batch = vec![
    ///     ("c1".to_string(), true, 90.0, true),
    ///     ("c2".to_string(), false, 85.0, false),
    ///     ("c3".to_string(), true, 80.0, false),
    /// ];
    ///
    /// let result = oracle.batch_learn(feedback_batch);
    /// assert_eq!(result.total_processed, 3);
    /// ```
    pub fn batch_learn(
        &mut self,
        feedback_batch: Vec<(String, bool, f64, bool)>,
    ) -> BatchLearningResult {
        if !self.config.enable_learning {
            return BatchLearningResult {
                total_processed: 0,
                correct_predictions: 0,
                incorrect_predictions: 0,
                accuracy_improvement: 0.0,
                model_weight_changes: HashMap::new(),
            };
        }

        let initial_stats = self.get_learning_stats();
        let mut correct = 0;
        let mut incorrect = 0;

        // Collect model weight changes
        let mut weight_changes: HashMap<String, (f64, f64)> = HashMap::new();
        for weight in &self.config.model_weights {
            weight_changes.insert(weight.model.clone(), (weight.weight, weight.weight));
        }

        // Process each feedback entry
        for (commitment_id, oracle_decision, oracle_confidence, admin_decision) in feedback_batch {
            let is_correct = oracle_decision == admin_decision;

            if is_correct {
                correct += 1;
            } else {
                incorrect += 1;
            }

            // Update model weights
            for weight in &mut self.config.model_weights {
                weight.update_accuracy(is_correct);
                if let Some(change) = weight_changes.get_mut(&weight.model) {
                    change.1 = weight.weight;
                }
            }

            // Record feedback
            self.feedback_log.push(FeedbackEntry {
                commitment_id,
                oracle_decision,
                oracle_confidence,
                admin_decision,
                timestamp: chrono::Utc::now(),
            });
        }

        let final_stats = self.get_learning_stats();
        let accuracy_improvement = final_stats.accuracy - initial_stats.accuracy;

        tracing::info!(
            total = correct + incorrect,
            correct,
            incorrect,
            accuracy_improvement,
            "Batch learning completed"
        );

        BatchLearningResult {
            total_processed: correct + incorrect,
            correct_predictions: correct,
            incorrect_predictions: incorrect,
            accuracy_improvement,
            model_weight_changes: weight_changes,
        }
    }

    /// Import feedback from CSV file
    ///
    /// Expected CSV format: `commitment_id,oracle_decision,oracle_confidence,admin_decision,timestamp`
    pub fn import_feedback_from_csv(&mut self, path: impl AsRef<std::path::Path>) -> Result<usize> {
        let csv_content = std::fs::read_to_string(path)
            .map_err(|e| AiError::Internal(format!("Failed to read CSV: {e}")))?;

        let mut imported = 0;
        let mut feedback_batch = Vec::new();

        for (i, line) in csv_content.lines().enumerate() {
            // Skip header
            if i == 0 {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 4 {
                continue;
            }

            let commitment_id = parts[0].to_string();
            let oracle_decision = parts[1]
                .parse::<bool>()
                .map_err(|_| AiError::ParseError(format!("Invalid boolean in line {}", i + 1)))?;
            let oracle_confidence = parts[2]
                .parse::<f64>()
                .map_err(|_| AiError::ParseError(format!("Invalid float in line {}", i + 1)))?;
            let admin_decision = parts[3]
                .parse::<bool>()
                .map_err(|_| AiError::ParseError(format!("Invalid boolean in line {}", i + 1)))?;

            feedback_batch.push((
                commitment_id,
                oracle_decision,
                oracle_confidence,
                admin_decision,
            ));
            imported += 1;
        }

        // Process batch
        self.batch_learn(feedback_batch);

        Ok(imported)
    }

    /// Analyze feedback patterns to identify systematic errors
    #[must_use]
    pub fn analyze_feedback_patterns(&self) -> FeedbackAnalysis {
        let mut high_conf_errors = 0;
        let mut low_conf_correct = 0;
        let mut false_positives = 0;
        let mut false_negatives = 0;

        for feedback in &self.feedback_log {
            let is_correct = feedback.oracle_decision == feedback.admin_decision;

            if !is_correct && feedback.oracle_confidence > 80.0 {
                high_conf_errors += 1;
            }

            if is_correct && feedback.oracle_confidence < 60.0 {
                low_conf_correct += 1;
            }

            if feedback.oracle_decision && !feedback.admin_decision {
                false_positives += 1;
            } else if !feedback.oracle_decision && feedback.admin_decision {
                false_negatives += 1;
            }
        }

        let total = self.feedback_log.len();

        FeedbackAnalysis {
            total_feedback: total,
            high_confidence_errors: high_conf_errors,
            low_confidence_correct: low_conf_correct,
            false_positives,
            false_negatives,
            false_positive_rate: if total > 0 {
                (false_positives as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            false_negative_rate: if total > 0 {
                (false_negatives as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Oracle state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OracleState {
    config: OracleConfig,
    feedback_log: Vec<FeedbackEntry>,
}

/// Automated decision result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoDecision {
    /// Automatically approve
    AutoApprove,
    /// Automatically reject
    AutoReject,
    /// Needs human review
    NeedsHumanReview,
}

/// Learning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStats {
    /// Total feedback entries
    pub total_feedback: usize,
    /// Correct decisions
    pub correct_decisions: usize,
    /// Overall accuracy (%)
    pub accuracy: f64,
    /// Accuracy by model
    pub model_accuracies: HashMap<String, f64>,
}

/// Result of batch learning operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchLearningResult {
    /// Total feedback entries processed
    pub total_processed: usize,
    /// Number of correct predictions
    pub correct_predictions: usize,
    /// Number of incorrect predictions
    pub incorrect_predictions: usize,
    /// Accuracy improvement from batch learning (percentage points)
    pub accuracy_improvement: f64,
    /// Model weight changes (`model_name` -> (before, after))
    pub model_weight_changes: HashMap<String, (f64, f64)>,
}

/// Feedback pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackAnalysis {
    /// Total feedback entries
    pub total_feedback: usize,
    /// High confidence errors (>80% confidence but wrong)
    pub high_confidence_errors: usize,
    /// Low confidence correct (< 60% confidence but correct)
    pub low_confidence_correct: usize,
    /// False positives (approved but should reject)
    pub false_positives: usize,
    /// False negatives (rejected but should approve)
    pub false_negatives: usize,
    /// False positive rate (%)
    pub false_positive_rate: f64,
    /// False negative rate (%)
    pub false_negative_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_weight_update() {
        let mut weight = ModelWeight::new("test-model", 0.5);
        assert_eq!(weight.accuracy, 0.5);

        // Simulate correct predictions
        for _ in 0..10 {
            weight.update_accuracy(true);
        }
        assert!(weight.accuracy > 0.5);
        assert!(weight.weight > 0.5);

        // Simulate incorrect predictions
        for _ in 0..10 {
            weight.update_accuracy(false);
        }
        assert!(weight.accuracy < 0.9);
    }

    #[test]
    fn test_unanimous_consensus() {
        let oracle = AiOracle::new(OracleConfig::default());

        let votes = vec![
            ModelVote {
                model: "model1".to_string(),
                approved: true,
                confidence: 90.0,
                reasoning: "Good".to_string(),
            },
            ModelVote {
                model: "model2".to_string(),
                approved: true,
                confidence: 85.0,
                reasoning: "Good".to_string(),
            },
        ];

        let (approved, confidence, _) = oracle.unanimous_consensus(&votes).unwrap();
        assert!(approved);
        assert_eq!(confidence, 87.5);
    }

    #[test]
    fn test_majority_consensus() {
        let oracle = AiOracle::new(OracleConfig::default());

        let votes = vec![
            ModelVote {
                model: "model1".to_string(),
                approved: true,
                confidence: 90.0,
                reasoning: "Good".to_string(),
            },
            ModelVote {
                model: "model2".to_string(),
                approved: true,
                confidence: 85.0,
                reasoning: "Good".to_string(),
            },
            ModelVote {
                model: "model3".to_string(),
                approved: false,
                confidence: 80.0,
                reasoning: "Bad".to_string(),
            },
        ];

        let (approved, _, _) = oracle.majority_consensus(&votes).unwrap();
        assert!(approved);
    }

    #[test]
    fn test_auto_decision() {
        let oracle = AiOracle::new(OracleConfig::default());

        let consensus = ConsensusResult {
            approved: true,
            confidence: 90.0,
            votes: vec![],
            strategy: ConsensusStrategy::Unanimous,
            reasoning: "Test".to_string(),
        };

        assert_eq!(
            oracle.can_auto_decide(&consensus),
            AutoDecision::AutoApprove
        );

        let consensus_low = ConsensusResult {
            approved: true,
            confidence: 70.0,
            votes: vec![],
            strategy: ConsensusStrategy::Unanimous,
            reasoning: "Test".to_string(),
        };

        assert_eq!(
            oracle.can_auto_decide(&consensus_low),
            AutoDecision::NeedsHumanReview
        );
    }

    #[test]
    fn test_oracle_persistence() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        // Add some feedback
        oracle.record_feedback("commitment1".to_string(), true, 90.0, true);
        oracle.record_feedback("commitment2".to_string(), false, 85.0, false);
        oracle.record_feedback("commitment3".to_string(), true, 80.0, false);

        // Save to temp file
        let temp_path = "/tmp/oracle_test_state.json";
        oracle.save_to_file(temp_path).unwrap();

        // Create new oracle and load
        let mut oracle2 = AiOracle::new(OracleConfig::default());
        oracle2.load_from_file(temp_path).unwrap();

        // Verify feedback log was restored
        assert_eq!(oracle2.get_feedback_log().len(), 3);
        assert_eq!(oracle2.get_feedback_log()[0].commitment_id, "commitment1");

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_feedback_csv_export() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        oracle.record_feedback("c1".to_string(), true, 90.0, true);
        oracle.record_feedback("c2".to_string(), false, 85.0, false);

        let temp_path = "/tmp/oracle_feedback.csv";
        oracle.export_feedback_csv(temp_path).unwrap();

        let csv_content = std::fs::read_to_string(temp_path).unwrap();
        assert!(csv_content.contains("commitment_id"));
        assert!(csv_content.contains("c1"));
        assert!(csv_content.contains("c2"));

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_feedback_log_operations() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        assert_eq!(oracle.get_feedback_log().len(), 0);

        oracle.record_feedback("c1".to_string(), true, 90.0, true);
        assert_eq!(oracle.get_feedback_log().len(), 1);

        oracle.clear_feedback_log();
        assert_eq!(oracle.get_feedback_log().len(), 0);
    }

    #[test]
    fn test_batch_learn() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        let feedback_batch = vec![
            ("c1".to_string(), true, 90.0, true),
            ("c2".to_string(), false, 85.0, false),
            ("c3".to_string(), true, 80.0, false),
            ("c4".to_string(), true, 92.0, true),
        ];

        let result = oracle.batch_learn(feedback_batch);

        assert_eq!(result.total_processed, 4);
        assert_eq!(result.correct_predictions, 3);
        assert_eq!(result.incorrect_predictions, 1);
        assert_eq!(oracle.get_feedback_log().len(), 4);
    }

    #[test]
    fn test_feedback_analysis() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        // Add various feedback types
        oracle.record_feedback("c1".to_string(), true, 95.0, false); // High conf error
        oracle.record_feedback("c2".to_string(), false, 55.0, false); // Low conf correct
        oracle.record_feedback("c3".to_string(), true, 80.0, false); // False positive
        oracle.record_feedback("c4".to_string(), false, 70.0, true); // False negative

        let analysis = oracle.analyze_feedback_patterns();

        assert_eq!(analysis.total_feedback, 4);
        assert_eq!(analysis.high_confidence_errors, 1);
        assert_eq!(analysis.low_confidence_correct, 1);
        assert_eq!(analysis.false_positives, 2);
        assert_eq!(analysis.false_negatives, 1);
    }

    #[test]
    fn test_csv_import() {
        let mut oracle = AiOracle::new(OracleConfig::default());

        // Create test CSV
        let csv_content = "commitment_id,oracle_decision,oracle_confidence,admin_decision,timestamp\n\
                           c1,true,90.0,true,2024-01-01T00:00:00Z\n\
                           c2,false,85.0,false,2024-01-02T00:00:00Z\n\
                           c3,true,80.0,false,2024-01-03T00:00:00Z";

        let temp_path = "/tmp/oracle_import_test.csv";
        std::fs::write(temp_path, csv_content).unwrap();

        let imported = oracle.import_feedback_from_csv(temp_path).unwrap();

        assert_eq!(imported, 3);
        assert_eq!(oracle.get_feedback_log().len(), 3);

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }
}
