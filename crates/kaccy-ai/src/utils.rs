//! Utility functions and helpers for common AI operations
//!
//! This module provides convenient helper functions that simplify
//! common patterns when using the kaccy-ai crate.

use crate::ai_evaluator::{FraudCheckRequest, VerificationRequest};
use crate::error::{AiError, Result};

/// Builder for creating `VerificationRequest` with sensible defaults
pub struct VerificationRequestBuilder {
    title: String,
    description: Option<String>,
    deadline: String,
    evidence_url: String,
    evidence_description: Option<String>,
}

impl VerificationRequestBuilder {
    /// Create a new verification request builder
    pub fn new(title: impl Into<String>, evidence_url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            deadline: String::new(),
            evidence_url: evidence_url.into(),
            evidence_description: None,
        }
    }

    /// Set the commitment description
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the deadline
    #[must_use]
    pub fn deadline(mut self, deadline: impl Into<String>) -> Self {
        self.deadline = deadline.into();
        self
    }

    /// Set the evidence description
    #[must_use]
    pub fn evidence_description(mut self, desc: impl Into<String>) -> Self {
        self.evidence_description = Some(desc.into());
        self
    }

    /// Build the verification request
    #[must_use]
    pub fn build(self) -> VerificationRequest {
        VerificationRequest {
            commitment_title: self.title,
            commitment_description: self.description,
            deadline: self.deadline,
            evidence_url: self.evidence_url,
            evidence_description: self.evidence_description,
        }
    }
}

/// Builder for creating `FraudCheckRequest` with sensible defaults
pub struct FraudCheckRequestBuilder {
    content_type: String,
    content: String,
    commitments_made: i32,
    commitments_fulfilled: i32,
    avg_quality_score: Option<f64>,
}

impl FraudCheckRequestBuilder {
    /// Create a new fraud check request builder
    pub fn new(content_type: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            content_type: content_type.into(),
            content: content.into(),
            commitments_made: 0,
            commitments_fulfilled: 0,
            avg_quality_score: None,
        }
    }

    /// Set commitments made
    #[must_use]
    pub fn commitments_made(mut self, count: i32) -> Self {
        self.commitments_made = count;
        self
    }

    /// Set commitments fulfilled
    #[must_use]
    pub fn commitments_fulfilled(mut self, count: i32) -> Self {
        self.commitments_fulfilled = count;
        self
    }

    /// Set average quality score
    #[must_use]
    pub fn avg_quality_score(mut self, score: f64) -> Self {
        self.avg_quality_score = Some(score);
        self
    }

    /// Build the fraud check request
    #[must_use]
    pub fn build(self) -> FraudCheckRequest {
        FraudCheckRequest {
            content_type: self.content_type,
            content: self.content,
            commitments_made: self.commitments_made,
            commitments_fulfilled: self.commitments_fulfilled,
            avg_quality_score: self.avg_quality_score,
        }
    }
}

/// Validate a URL format
pub fn validate_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Err(AiError::InvalidInput("URL cannot be empty".to_string()));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AiError::InvalidInput(
            "URL must start with http:// or https://".to_string(),
        ));
    }

    Ok(())
}

/// Validate a confidence score is within valid range (0-100)
pub fn validate_confidence(score: f64) -> Result<()> {
    if !(0.0..=100.0).contains(&score) {
        return Err(AiError::InvalidInput(format!(
            "Confidence score must be between 0 and 100, got {score}"
        )));
    }
    Ok(())
}

/// Validate a quality score is within valid range (0-100)
pub fn validate_quality_score(score: f64) -> Result<()> {
    if !(0.0..=100.0).contains(&score) {
        return Err(AiError::InvalidInput(format!(
            "Quality score must be between 0 and 100, got {score}"
        )));
    }
    Ok(())
}

/// Calculate success rate from counts
#[must_use]
pub fn calculate_success_rate(successes: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    successes as f64 / total as f64
}

/// Format a duration in human-readable form
#[must_use]
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format a cost amount in USD with appropriate precision
#[must_use]
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${cost:.6}")
    } else if cost < 1.0 {
        format!("${cost:.4}")
    } else {
        format!("${cost:.2}")
    }
}

/// Calculate average from a slice of f64 values
#[must_use]
pub fn calculate_average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Calculate median from a slice of f64 values
#[must_use]
pub fn calculate_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        f64::midpoint(sorted[mid - 1], sorted[mid])
    } else {
        sorted[mid]
    }
}

/// Calculate standard deviation from a slice of f64 values
#[must_use]
pub fn calculate_std_dev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let avg = calculate_average(values);
    let variance = values
        .iter()
        .map(|v| {
            let diff = v - avg;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;

    variance.sqrt()
}

/// Clamp a value between min and max
pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Normalize a score from one range to another
#[must_use]
pub fn normalize_score(score: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let normalized = (score - from_min) / (from_max - from_min);
    to_min + normalized * (to_max - to_min)
}

/// Check if a score represents a passing grade (>= 70%)
#[must_use]
pub fn is_passing_score(score: f64) -> bool {
    score >= 70.0
}

/// Check if a score represents excellence (>= 90%)
#[must_use]
pub fn is_excellent_score(score: f64) -> bool {
    score >= 90.0
}

/// Convert a confidence percentage to a risk level description
#[must_use]
pub fn confidence_to_risk_level(confidence: f64) -> &'static str {
    if confidence >= 90.0 {
        "Very Low Risk"
    } else if confidence >= 75.0 {
        "Low Risk"
    } else if confidence >= 60.0 {
        "Medium Risk"
    } else if confidence >= 40.0 {
        "High Risk"
    } else {
        "Very High Risk"
    }
}

/// Retry a function with exponential backoff
///
/// # Example
/// ```no_run
/// use kaccy_ai::utils::retry_with_exponential_backoff;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let result = retry_with_exponential_backoff(
///     3,
///     std::time::Duration::from_millis(100),
///     || async {
///         // Your async operation here
///         Ok::<_, std::io::Error>(42)
///     }
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn retry_with_exponential_backoff<F, Fut, T, E>(
    max_retries: u32,
    initial_delay: std::time::Duration,
    mut f: F,
) -> std::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
{
    let mut delay = initial_delay;
    let mut attempts = 0;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts >= max_retries {
                    return Err(e);
                }
                tokio::time::sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }
    }
}

// Additional statistical utilities

/// Calculate percentile from a slice of f64 values
///
/// # Arguments
/// * `values` - Slice of values to calculate percentile from
/// * `percentile` - Percentile to calculate (0-100)
///
/// # Returns
/// The value at the given percentile, or 0.0 if values is empty
#[must_use]
pub fn calculate_percentile(values: &[f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let clamped_percentile = clamp(percentile, 0.0, 100.0);
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let index = (clamped_percentile / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Calculate weighted average from slices of values and weights
///
/// # Arguments
/// * `values` - Slice of values
/// * `weights` - Slice of weights (must be same length as values)
///
/// # Returns
/// Weighted average, or 0.0 if inputs are empty or lengths don't match
#[must_use]
pub fn calculate_weighted_average(values: &[f64], weights: &[f64]) -> f64 {
    if values.is_empty() || weights.is_empty() || values.len() != weights.len() {
        return 0.0;
    }

    let total_weight: f64 = weights.iter().sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    values
        .iter()
        .zip(weights.iter())
        .map(|(v, w)| v * w)
        .sum::<f64>()
        / total_weight
}

/// Calculate variance from a slice of f64 values
#[must_use]
pub fn calculate_variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let avg = calculate_average(values);
    values
        .iter()
        .map(|v| {
            let diff = v - avg;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64
}

/// Calculate coefficient of variation (CV) - std dev as percentage of mean
/// Useful for comparing variability across datasets with different scales
#[must_use]
pub fn calculate_coefficient_of_variation(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let avg = calculate_average(values);
    if avg == 0.0 {
        return 0.0;
    }

    let std_dev = calculate_std_dev(values);
    (std_dev / avg) * 100.0
}

// Formatting utilities

/// Format token count in human-readable form
#[must_use]
pub fn format_tokens(count: usize) -> String {
    if count < 1000 {
        format!("{count} tokens")
    } else if count < 1_000_000 {
        format!("{:.1}K tokens", count as f64 / 1000.0)
    } else {
        format!("{:.1}M tokens", count as f64 / 1_000_000.0)
    }
}

/// Format file size in human-readable form
#[must_use]
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

/// Format percentage with appropriate precision
#[must_use]
pub fn format_percentage(value: f64) -> String {
    if value < 1.0 {
        format!("{value:.2}%")
    } else if value < 10.0 {
        format!("{value:.1}%")
    } else {
        format!("{value:.0}%")
    }
}

// Validation utilities

/// Validate token count is within reasonable range
pub fn validate_token_count(count: usize, max_tokens: usize) -> Result<()> {
    if count == 0 {
        return Err(AiError::InvalidInput(
            "Token count cannot be zero".to_string(),
        ));
    }

    if count > max_tokens {
        return Err(AiError::InvalidInput(format!(
            "Token count {count} exceeds maximum of {max_tokens}"
        )));
    }

    Ok(())
}

/// Validate temperature parameter for LLM requests
pub fn validate_temperature(temperature: f64) -> Result<()> {
    if !(0.0..=2.0).contains(&temperature) {
        return Err(AiError::InvalidInput(format!(
            "Temperature must be between 0.0 and 2.0, got {temperature}"
        )));
    }
    Ok(())
}

/// Validate model name is not empty
pub fn validate_model_name(model: &str) -> Result<()> {
    if model.trim().is_empty() {
        return Err(AiError::InvalidInput(
            "Model name cannot be empty".to_string(),
        ));
    }
    Ok(())
}

// Score aggregation utilities

/// Aggregate multiple scores using different strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationStrategy {
    /// Take the average of all scores
    Average,
    /// Take the median of all scores
    Median,
    /// Take the minimum score (most conservative)
    Minimum,
    /// Take the maximum score (most optimistic)
    Maximum,
    /// Take the weighted average (requires weights)
    Weighted,
}

/// Aggregate scores using the specified strategy
pub fn aggregate_scores(
    scores: &[f64],
    strategy: AggregationStrategy,
    weights: Option<&[f64]>,
) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }

    match strategy {
        AggregationStrategy::Average => calculate_average(scores),
        AggregationStrategy::Median => calculate_median(scores),
        AggregationStrategy::Minimum => scores.iter().copied().fold(f64::INFINITY, f64::min),
        AggregationStrategy::Maximum => scores.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        AggregationStrategy::Weighted => {
            if let Some(w) = weights {
                calculate_weighted_average(scores, w)
            } else {
                calculate_average(scores)
            }
        }
    }
}

/// Combine quality and originality scores into a final score
/// Uses weighted average: quality (60%) + originality (40%)
#[must_use]
pub fn combine_quality_originality(quality: f64, originality: f64) -> f64 {
    quality * 0.6 + originality * 0.4
}

/// Calculate consensus score from multiple evaluations
/// Returns average score and confidence based on agreement
#[must_use]
pub fn calculate_consensus(scores: &[f64]) -> (f64, f64) {
    if scores.is_empty() {
        return (0.0, 0.0);
    }

    let avg = calculate_average(scores);
    let std_dev = calculate_std_dev(scores);

    // Lower standard deviation means higher confidence
    // Map std dev to confidence: 0 std dev = 100% confidence, high std dev = low confidence
    let confidence = if std_dev < 5.0 {
        100.0
    } else if std_dev < 10.0 {
        90.0 - (std_dev - 5.0) * 4.0
    } else if std_dev < 20.0 {
        70.0 - (std_dev - 10.0) * 2.0
    } else {
        clamp(50.0 - (std_dev - 20.0), 0.0, 50.0)
    };

    (avg, confidence)
}

// Comparison utilities

/// Compare two scores and return the difference as a percentage
#[must_use]
pub fn score_difference_percent(score1: f64, score2: f64) -> f64 {
    if score2 == 0.0 {
        return 0.0;
    }
    ((score1 - score2) / score2) * 100.0
}

/// Determine if two scores are significantly different (> 10% difference)
#[must_use]
pub fn scores_significantly_different(score1: f64, score2: f64) -> bool {
    let diff_percent = score_difference_percent(score1, score2).abs();
    diff_percent > 10.0
}

/// Get score grade letter (A, B, C, D, F)
#[must_use]
pub fn score_to_grade(score: f64) -> char {
    if score >= 90.0 {
        'A'
    } else if score >= 80.0 {
        'B'
    } else if score >= 70.0 {
        'C'
    } else if score >= 60.0 {
        'D'
    } else {
        'F'
    }
}

/// Get score tier description
#[must_use]
pub fn score_to_tier(score: f64) -> &'static str {
    if score >= 95.0 {
        "Exceptional"
    } else if score >= 85.0 {
        "Excellent"
    } else if score >= 75.0 {
        "Good"
    } else if score >= 65.0 {
        "Fair"
    } else if score >= 50.0 {
        "Poor"
    } else {
        "Very Poor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_request_builder() {
        let request = VerificationRequestBuilder::new("Test Commitment", "https://example.com")
            .description("Test description")
            .deadline("2024-12-31")
            .evidence_description("Evidence desc")
            .build();

        assert_eq!(request.commitment_title, "Test Commitment");
        assert_eq!(
            request.commitment_description,
            Some("Test description".to_string())
        );
        assert_eq!(request.deadline, "2024-12-31");
        assert_eq!(request.evidence_url, "https://example.com");
    }

    #[test]
    fn test_fraud_check_request_builder() {
        let request = FraudCheckRequestBuilder::new("Test Type", "Test Content")
            .commitments_made(10)
            .commitments_fulfilled(8)
            .avg_quality_score(85.0)
            .build();

        assert_eq!(request.content_type, "Test Type");
        assert_eq!(request.commitments_made, 10);
        assert_eq!(request.commitments_fulfilled, 8);
        assert_eq!(request.avg_quality_score, Some(85.0));
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://example.com").is_ok());
        assert!(validate_url("").is_err());
        assert!(validate_url("example.com").is_err());
    }

    #[test]
    fn test_validate_confidence() {
        assert!(validate_confidence(50.0).is_ok());
        assert!(validate_confidence(0.0).is_ok());
        assert!(validate_confidence(100.0).is_ok());
        assert!(validate_confidence(-1.0).is_err());
        assert!(validate_confidence(101.0).is_err());
    }

    #[test]
    fn test_calculate_success_rate() {
        assert!((calculate_success_rate(7, 10) - 0.7).abs() < 1e-10);
        assert!((calculate_success_rate(10, 10) - 1.0).abs() < 1e-10);
        assert!((calculate_success_rate(0, 10)).abs() < 1e-10);
        assert!((calculate_success_rate(0, 0)).abs() < 1e-10);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(std::time::Duration::from_secs(30)), "30s");
        assert_eq!(
            format_duration(std::time::Duration::from_secs(90)),
            "1m 30s"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_secs(3661)),
            "1h 1m"
        );
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.001), "$0.001000");
        assert_eq!(format_cost(0.05), "$0.0500");
        assert_eq!(format_cost(1.50), "$1.50");
    }

    #[test]
    fn test_calculate_average() {
        assert!((calculate_average(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-10);
        assert!((calculate_average(&[])).abs() < 1e-10);
        assert!((calculate_average(&[5.0]) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_median() {
        assert!((calculate_median(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-10);
        assert!((calculate_median(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 1e-10);
        assert!((calculate_median(&[])).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_std_dev() {
        let values = vec![2.0, 4.0, 6.0, 8.0];
        let std_dev = calculate_std_dev(&values);
        assert!((std_dev - 2.236).abs() < 0.01);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(15, 0, 10), 10);
    }

    #[test]
    fn test_normalize_score() {
        assert!((normalize_score(50.0, 0.0, 100.0, 0.0, 1.0) - 0.5).abs() < 1e-10);
        assert!((normalize_score(0.0, 0.0, 100.0, 0.0, 1.0)).abs() < 1e-10);
        assert!((normalize_score(100.0, 0.0, 100.0, 0.0, 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_passing_score() {
        assert!(is_passing_score(70.0));
        assert!(is_passing_score(85.0));
        assert!(!is_passing_score(69.9));
    }

    #[test]
    fn test_is_excellent_score() {
        assert!(is_excellent_score(90.0));
        assert!(is_excellent_score(95.0));
        assert!(!is_excellent_score(89.9));
    }

    #[test]
    fn test_confidence_to_risk_level() {
        assert_eq!(confidence_to_risk_level(95.0), "Very Low Risk");
        assert_eq!(confidence_to_risk_level(80.0), "Low Risk");
        assert_eq!(confidence_to_risk_level(65.0), "Medium Risk");
        assert_eq!(confidence_to_risk_level(50.0), "High Risk");
        assert_eq!(confidence_to_risk_level(30.0), "Very High Risk");
    }

    // Tests for new statistical utilities

    #[test]
    fn test_calculate_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((calculate_percentile(&values, 0.0) - 1.0).abs() < 1e-10);
        assert!((calculate_percentile(&values, 50.0) - 3.0).abs() < 1e-10);
        assert!((calculate_percentile(&values, 100.0) - 5.0).abs() < 1e-10);
        assert!((calculate_percentile(&[], 50.0)).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_weighted_average() {
        let values = vec![80.0, 90.0, 70.0];
        let weights = vec![0.5, 0.3, 0.2];
        let weighted_avg = calculate_weighted_average(&values, &weights);
        assert!((weighted_avg - 81.0).abs() < 1e-10);

        // Mismatched lengths
        assert!((calculate_weighted_average(&values, &[0.5, 0.5])).abs() < 1e-10);

        // Empty inputs
        assert!((calculate_weighted_average(&[], &[])).abs() < 1e-10);

        // Zero total weight
        assert!((calculate_weighted_average(&values, &[0.0, 0.0, 0.0])).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_variance() {
        let values = vec![2.0, 4.0, 6.0, 8.0];
        let variance = calculate_variance(&values);
        assert!((variance - 5.0).abs() < 1e-10);

        assert!((calculate_variance(&[])).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_coefficient_of_variation() {
        let values = vec![10.0, 12.0, 14.0, 16.0];
        let cv = calculate_coefficient_of_variation(&values);
        assert!(cv > 0.0 && cv < 100.0);

        // Empty
        assert!((calculate_coefficient_of_variation(&[])).abs() < 1e-10);

        // Zero mean
        assert!((calculate_coefficient_of_variation(&[0.0, 0.0, 0.0])).abs() < 1e-10);
    }

    // Tests for formatting utilities

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500 tokens");
        assert_eq!(format_tokens(1500), "1.5K tokens");
        assert_eq!(format_tokens(1_500_000), "1.5M tokens");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(1_572_864), "1.50 MB");
        assert_eq!(format_file_size(1_610_612_736), "1.50 GB");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(0.5), "0.50%");
        assert_eq!(format_percentage(5.5), "5.5%");
        assert_eq!(format_percentage(55.5), "56%");
    }

    // Tests for validation utilities

    #[test]
    fn test_validate_token_count() {
        assert!(validate_token_count(100, 1000).is_ok());
        assert!(validate_token_count(0, 1000).is_err());
        assert!(validate_token_count(1001, 1000).is_err());
    }

    #[test]
    fn test_validate_temperature() {
        assert!(validate_temperature(0.7).is_ok());
        assert!(validate_temperature(0.0).is_ok());
        assert!(validate_temperature(2.0).is_ok());
        assert!(validate_temperature(-0.1).is_err());
        assert!(validate_temperature(2.1).is_err());
    }

    #[test]
    fn test_validate_model_name() {
        assert!(validate_model_name("gpt-4").is_ok());
        assert!(validate_model_name("").is_err());
        assert!(validate_model_name("   ").is_err());
    }

    // Tests for aggregation utilities

    #[test]
    fn test_aggregate_scores() {
        let scores = vec![70.0, 80.0, 90.0];

        assert!(
            (aggregate_scores(&scores, AggregationStrategy::Average, None) - 80.0).abs() < 1e-10
        );
        assert!(
            (aggregate_scores(&scores, AggregationStrategy::Median, None) - 80.0).abs() < 1e-10
        );
        assert!(
            (aggregate_scores(&scores, AggregationStrategy::Minimum, None) - 70.0).abs() < 1e-10
        );
        assert!(
            (aggregate_scores(&scores, AggregationStrategy::Maximum, None) - 90.0).abs() < 1e-10
        );

        let weights = vec![0.2, 0.3, 0.5];
        // 70 * 0.2 + 80 * 0.3 + 90 * 0.5 = 14 + 24 + 45 = 83.0
        assert!(
            (aggregate_scores(&scores, AggregationStrategy::Weighted, Some(&weights)) - 83.0).abs()
                < 1e-10
        );

        // Empty scores
        assert!((aggregate_scores(&[], AggregationStrategy::Average, None)).abs() < 1e-10);
    }

    #[test]
    fn test_combine_quality_originality() {
        let quality = 80.0;
        let originality = 90.0;
        let combined = combine_quality_originality(quality, originality);
        assert!((combined - 84.0).abs() < 1e-10); // 80 * 0.6 + 90 * 0.4
    }

    #[test]
    fn test_calculate_consensus() {
        // High agreement (low std dev) -> high confidence
        let scores = vec![85.0, 86.0, 84.0, 85.5];
        let (avg, confidence) = calculate_consensus(&scores);
        assert!((avg - 85.125).abs() < 0.1);
        assert!(confidence > 90.0);

        // Low agreement (high std dev) -> low confidence
        let scores2 = vec![50.0, 90.0, 60.0, 80.0];
        let (avg2, confidence2) = calculate_consensus(&scores2);
        assert!((avg2 - 70.0).abs() < 1e-10);
        assert!(confidence2 < 80.0);

        // Empty
        let (avg3, confidence3) = calculate_consensus(&[]);
        assert!((avg3).abs() < 1e-10);
        assert!((confidence3).abs() < 1e-10);
    }

    // Tests for comparison utilities

    #[test]
    fn test_score_difference_percent() {
        assert!((score_difference_percent(110.0, 100.0) - 10.0).abs() < 1e-10);
        assert!((score_difference_percent(90.0, 100.0) + 10.0).abs() < 1e-10);
        assert!((score_difference_percent(100.0, 0.0)).abs() < 1e-10);
    }

    #[test]
    fn test_scores_significantly_different() {
        assert!(scores_significantly_different(100.0, 80.0)); // 25% difference
        assert!(scores_significantly_different(100.0, 89.0)); // 12.4% difference
        assert!(!scores_significantly_different(100.0, 95.0)); // 5.3% difference
    }

    #[test]
    fn test_score_to_grade() {
        assert_eq!(score_to_grade(95.0), 'A');
        assert_eq!(score_to_grade(85.0), 'B');
        assert_eq!(score_to_grade(75.0), 'C');
        assert_eq!(score_to_grade(65.0), 'D');
        assert_eq!(score_to_grade(50.0), 'F');
    }

    #[test]
    fn test_score_to_tier() {
        assert_eq!(score_to_tier(96.0), "Exceptional");
        assert_eq!(score_to_tier(87.0), "Excellent");
        assert_eq!(score_to_tier(77.0), "Good");
        assert_eq!(score_to_tier(67.0), "Fair");
        assert_eq!(score_to_tier(52.0), "Poor");
        assert_eq!(score_to_tier(40.0), "Very Poor");
    }
}
