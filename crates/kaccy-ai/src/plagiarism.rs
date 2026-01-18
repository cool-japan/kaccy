//! Plagiarism detection for code and text content
//!
//! Provides similarity detection using multiple techniques:
//! - Token-based code similarity
//! - Character n-gram text similarity
//! - LLM-powered semantic similarity
//! - Fuzzy matching for near-duplicates

use crate::error::AiError;
use crate::llm::{ChatMessage, ChatRequest, ChatRole, LlmClient};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Plagiarism detection configuration
#[derive(Debug, Clone)]
pub struct PlagiarismConfig {
    /// Minimum similarity threshold (0.0-1.0) to flag as plagiarism
    pub similarity_threshold: f64,
    /// Use LLM for semantic similarity analysis
    pub use_semantic_analysis: bool,
    /// N-gram size for text comparison
    pub ngram_size: usize,
    /// Minimum token overlap for code similarity
    pub min_token_overlap: usize,
}

impl Default for PlagiarismConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            use_semantic_analysis: true,
            ngram_size: 3,
            min_token_overlap: 10,
        }
    }
}

/// Result of plagiarism detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlagiarismResult {
    /// Overall similarity score (0.0-1.0)
    pub similarity_score: f64,
    /// Whether content is flagged as plagiarized
    pub is_plagiarized: bool,
    /// Detailed similarity breakdown
    pub details: SimilarityDetails,
    /// Explanation of the detection
    pub explanation: String,
    /// Confidence in the detection (0-100)
    pub confidence: u32,
}

/// Detailed similarity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityDetails {
    /// Token-based similarity (for code)
    pub token_similarity: f64,
    /// N-gram similarity (for text)
    pub ngram_similarity: f64,
    /// Semantic similarity from LLM (if enabled)
    pub semantic_similarity: Option<f64>,
    /// Matching tokens or phrases
    pub matches: Vec<String>,
    /// Total tokens/n-grams compared
    pub total_comparisons: usize,
}

/// Plagiarism detector
pub struct PlagiarismDetector {
    config: PlagiarismConfig,
    llm_client: Option<LlmClient>,
}

impl PlagiarismDetector {
    /// Create a new plagiarism detector
    #[must_use]
    pub fn new(config: PlagiarismConfig) -> Self {
        Self {
            config,
            llm_client: None,
        }
    }

    /// Create with LLM client for semantic analysis
    #[must_use]
    pub fn with_llm(config: PlagiarismConfig, llm_client: LlmClient) -> Self {
        Self {
            config,
            llm_client: Some(llm_client),
        }
    }

    /// Detect plagiarism between two code snippets
    pub async fn detect_code_plagiarism(
        &self,
        original: &str,
        submission: &str,
    ) -> Result<PlagiarismResult, AiError> {
        // Tokenize code
        let original_tokens = Self::tokenize_code(original);
        let submission_tokens = Self::tokenize_code(submission);

        // Calculate token similarity
        let token_similarity =
            Self::calculate_token_similarity(&original_tokens, &submission_tokens);

        // Calculate n-gram similarity as fallback
        let ngram_similarity = self.calculate_ngram_similarity(original, submission);

        // Get semantic similarity if LLM is available
        let semantic_similarity = if self.config.use_semantic_analysis && self.llm_client.is_some()
        {
            Some(
                self.analyze_semantic_similarity(original, submission, "code")
                    .await?,
            )
        } else {
            None
        };

        // Find matching tokens
        let matches = Self::find_matching_tokens(&original_tokens, &submission_tokens);

        // Calculate overall similarity
        let similarity_score = if let Some(semantic) = semantic_similarity {
            token_similarity * 0.4 + ngram_similarity * 0.2 + semantic * 0.4
        } else {
            token_similarity * 0.6 + ngram_similarity * 0.4
        };

        // Determine if plagiarized
        let is_plagiarized = similarity_score >= self.config.similarity_threshold;

        // Generate explanation
        let explanation = self.generate_explanation(
            similarity_score,
            is_plagiarized,
            token_similarity,
            ngram_similarity,
            semantic_similarity,
        );

        // Calculate confidence
        let confidence = self.calculate_confidence(
            token_similarity,
            ngram_similarity,
            semantic_similarity,
            matches.len(),
        );

        Ok(PlagiarismResult {
            similarity_score,
            is_plagiarized,
            details: SimilarityDetails {
                token_similarity,
                ngram_similarity,
                semantic_similarity,
                matches,
                total_comparisons: original_tokens.len().max(submission_tokens.len()),
            },
            explanation,
            confidence,
        })
    }

    /// Detect plagiarism between two text documents
    pub async fn detect_text_plagiarism(
        &self,
        original: &str,
        submission: &str,
    ) -> Result<PlagiarismResult, AiError> {
        // Calculate n-gram similarity
        let ngram_similarity = self.calculate_ngram_similarity(original, submission);

        // Calculate word-level token similarity
        let original_words = Self::tokenize_text(original);
        let submission_words = Self::tokenize_text(submission);
        let token_similarity = Self::calculate_token_similarity(&original_words, &submission_words);

        // Get semantic similarity if LLM is available
        let semantic_similarity = if self.config.use_semantic_analysis && self.llm_client.is_some()
        {
            Some(
                self.analyze_semantic_similarity(original, submission, "text")
                    .await?,
            )
        } else {
            None
        };

        // Find matching phrases
        let matches = Self::find_matching_tokens(&original_words, &submission_words);

        // Calculate overall similarity
        let similarity_score = if let Some(semantic) = semantic_similarity {
            ngram_similarity * 0.3 + token_similarity * 0.3 + semantic * 0.4
        } else {
            ngram_similarity * 0.5 + token_similarity * 0.5
        };

        // Determine if plagiarized
        let is_plagiarized = similarity_score >= self.config.similarity_threshold;

        // Generate explanation
        let explanation = self.generate_explanation(
            similarity_score,
            is_plagiarized,
            token_similarity,
            ngram_similarity,
            semantic_similarity,
        );

        // Calculate confidence
        let confidence = self.calculate_confidence(
            token_similarity,
            ngram_similarity,
            semantic_similarity,
            matches.len(),
        );

        Ok(PlagiarismResult {
            similarity_score,
            is_plagiarized,
            details: SimilarityDetails {
                token_similarity,
                ngram_similarity,
                semantic_similarity,
                matches,
                total_comparisons: original_words.len().max(submission_words.len()),
            },
            explanation,
            confidence,
        })
    }

    /// Tokenize code into meaningful tokens
    fn tokenize_code(code: &str) -> Vec<String> {
        code.split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect()
    }

    /// Tokenize text into words
    fn tokenize_text(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|s| {
                s.chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Calculate token-based similarity using Jaccard index
    fn calculate_token_similarity(original: &[String], submission: &[String]) -> f64 {
        let original_set: HashSet<_> = original.iter().collect();
        let submission_set: HashSet<_> = submission.iter().collect();

        let intersection = original_set.intersection(&submission_set).count();
        let union = original_set.union(&submission_set).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Calculate n-gram similarity
    fn calculate_ngram_similarity(&self, original: &str, submission: &str) -> f64 {
        let original_ngrams = self.extract_ngrams(original);
        let submission_ngrams = self.extract_ngrams(submission);

        let original_set: HashSet<_> = original_ngrams.iter().collect();
        let submission_set: HashSet<_> = submission_ngrams.iter().collect();

        let intersection = original_set.intersection(&submission_set).count();
        let union = original_set.union(&submission_set).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Extract character n-grams
    fn extract_ngrams(&self, text: &str) -> Vec<String> {
        let cleaned: String = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();

        if cleaned.len() < self.config.ngram_size {
            return vec![cleaned];
        }

        cleaned
            .chars()
            .collect::<Vec<_>>()
            .windows(self.config.ngram_size)
            .map(|window| window.iter().collect())
            .collect()
    }

    /// Find matching tokens between two token lists
    fn find_matching_tokens(original: &[String], submission: &[String]) -> Vec<String> {
        let original_set: HashSet<_> = original.iter().collect();
        let submission_set: HashSet<_> = submission.iter().collect();

        original_set
            .intersection(&submission_set)
            .take(20) // Limit to top 20 matches
            .map(|s| (*s).clone())
            .collect()
    }

    /// Analyze semantic similarity using LLM
    async fn analyze_semantic_similarity(
        &self,
        original: &str,
        submission: &str,
        content_type: &str,
    ) -> Result<f64, AiError> {
        let llm_client = self.llm_client.as_ref().ok_or_else(|| {
            AiError::Configuration("LLM client not configured for semantic analysis".to_string())
        })?;

        let prompt = format!(
            "Compare the following two {content_type} snippets and rate their semantic similarity on a scale of 0.0 to 1.0, where 1.0 is identical meaning and 0.0 is completely different. Only respond with a number.\n\nOriginal:\n{original}\n\nSubmission:\n{submission}\n\nSimilarity score:"
        );

        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: prompt,
            }],
            max_tokens: None,
            temperature: Some(0.0),
            stop: None,
            images: None,
        };

        let response = llm_client.chat(request).await?;
        let score_str = response.message.content.trim();

        // Parse the score
        score_str
            .parse::<f64>()
            .map_err(|_| {
                AiError::ParseError(format!("Failed to parse similarity score: {score_str}"))
            })
            .map(|score| score.clamp(0.0, 1.0))
    }

    /// Generate explanation for the detection result
    fn generate_explanation(
        &self,
        similarity_score: f64,
        is_plagiarized: bool,
        token_similarity: f64,
        ngram_similarity: f64,
        semantic_similarity: Option<f64>,
    ) -> String {
        let mut explanation = if is_plagiarized {
            format!(
                "Content flagged as plagiarized with {:.1}% overall similarity. ",
                similarity_score * 100.0
            )
        } else {
            format!(
                "Content not flagged as plagiarized ({:.1}% similarity is below threshold). ",
                similarity_score * 100.0
            )
        };

        let _ = write!(
            explanation,
            "Token similarity: {:.1}%, N-gram similarity: {:.1}%",
            token_similarity * 100.0,
            ngram_similarity * 100.0
        );

        if let Some(semantic) = semantic_similarity {
            let _ = write!(
                explanation,
                ", Semantic similarity: {:.1}%",
                semantic * 100.0
            );
        }

        explanation
    }

    /// Calculate confidence in the detection
    fn calculate_confidence(
        &self,
        token_similarity: f64,
        ngram_similarity: f64,
        semantic_similarity: Option<f64>,
        match_count: usize,
    ) -> u32 {
        let mut confidence = 0.0;

        // Base confidence from similarity consistency
        let similarities = if let Some(semantic) = semantic_similarity {
            vec![token_similarity, ngram_similarity, semantic]
        } else {
            vec![token_similarity, ngram_similarity]
        };

        let avg_similarity = similarities.iter().sum::<f64>() / similarities.len() as f64;
        let variance = similarities
            .iter()
            .map(|s| (s - avg_similarity).powi(2))
            .sum::<f64>()
            / similarities.len() as f64;

        // Higher confidence when metrics agree (low variance)
        let consistency_score = (1.0 - variance).max(0.0);
        confidence += consistency_score * 50.0;

        // Confidence from semantic analysis availability
        if semantic_similarity.is_some() {
            confidence += 30.0;
        }

        // Confidence from number of matches
        confidence += (match_count as f64 / 20.0).min(1.0) * 20.0;

        confidence.round() as u32
    }
}

/// Batch plagiarism detection
pub struct BatchPlagiarismDetector {
    detector: PlagiarismDetector,
}

impl BatchPlagiarismDetector {
    /// Create a new batch plagiarism detector
    #[must_use]
    pub fn new(detector: PlagiarismDetector) -> Self {
        Self { detector }
    }

    /// Check a submission against multiple known documents
    pub async fn check_against_corpus(
        &self,
        submission: &str,
        corpus: &[String],
        is_code: bool,
    ) -> Result<Vec<PlagiarismResult>, AiError> {
        let mut results = Vec::new();

        for original in corpus {
            let result = if is_code {
                self.detector
                    .detect_code_plagiarism(original, submission)
                    .await?
            } else {
                self.detector
                    .detect_text_plagiarism(original, submission)
                    .await?
            };

            if result.is_plagiarized {
                results.push(result);
            }
        }

        // Sort by similarity score (highest first)
        results.sort_by(|a, b| {
            b.similarity_score
                .partial_cmp(&a.similarity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    /// Find the most similar document in a corpus
    pub async fn find_most_similar(
        &self,
        submission: &str,
        corpus: &[String],
        is_code: bool,
    ) -> Result<Option<(usize, PlagiarismResult)>, AiError> {
        let mut best_match: Option<(usize, PlagiarismResult)> = None;

        for (idx, original) in corpus.iter().enumerate() {
            let result = if is_code {
                self.detector
                    .detect_code_plagiarism(original, submission)
                    .await?
            } else {
                self.detector
                    .detect_text_plagiarism(original, submission)
                    .await?
            };

            if let Some((_, ref current_best)) = best_match {
                if result.similarity_score > current_best.similarity_score {
                    best_match = Some((idx, result));
                }
            } else {
                best_match = Some((idx, result));
            }
        }

        Ok(best_match)
    }
}

/// Similarity matrix for multiple documents
pub struct SimilarityMatrix {
    scores: HashMap<(usize, usize), f64>,
    document_count: usize,
}

impl SimilarityMatrix {
    /// Create a new similarity matrix
    #[must_use]
    pub fn new(document_count: usize) -> Self {
        Self {
            scores: HashMap::new(),
            document_count,
        }
    }

    /// Add a similarity score
    pub fn add_score(&mut self, doc1: usize, doc2: usize, score: f64) {
        self.scores.insert((doc1, doc2), score);
    }

    /// Get similarity between two documents
    #[must_use]
    pub fn get_score(&self, doc1: usize, doc2: usize) -> Option<f64> {
        self.scores.get(&(doc1, doc2)).copied()
    }

    /// Find clusters of similar documents
    #[must_use]
    pub fn find_clusters(&self, threshold: f64) -> Vec<Vec<usize>> {
        let mut clusters: Vec<Vec<usize>> = Vec::new();
        let mut assigned: HashSet<usize> = HashSet::new();

        for i in 0..self.document_count {
            if assigned.contains(&i) {
                continue;
            }

            let mut cluster = vec![i];
            assigned.insert(i);

            for j in (i + 1)..self.document_count {
                if assigned.contains(&j) {
                    continue;
                }

                if let Some(score) = self.get_score(i, j) {
                    if score >= threshold {
                        cluster.push(j);
                        assigned.insert(j);
                    }
                }
            }

            if cluster.len() > 1 {
                clusters.push(cluster);
            }
        }

        clusters
    }
}

/// Plagiarism report generator
pub struct PlagiarismReport;

impl PlagiarismReport {
    /// Generate a detailed markdown report
    #[must_use]
    pub fn generate_markdown(
        result: &PlagiarismResult,
        original_id: &str,
        submission_id: &str,
    ) -> String {
        let mut report = String::new();

        report.push_str("# Plagiarism Detection Report\n\n");
        let _ = writeln!(report, "**Original:** {original_id}");
        let _ = writeln!(report, "**Submission:** {submission_id}");

        // Verdict
        report.push_str("## Verdict\n\n");
        if result.is_plagiarized {
            report.push_str("⚠️ **PLAGIARISM DETECTED**\n\n");
        } else {
            report.push_str("✓ **No Plagiarism Detected**\n\n");
        }

        // Overall metrics
        report.push_str("## Overall Metrics\n\n");
        let _ = writeln!(
            report,
            "- **Similarity Score:** {:.1}%",
            result.similarity_score * 100.0
        );
        let _ = writeln!(report, "- **Confidence:** {}%", result.confidence);
        let _ = writeln!(
            report,
            "- **Total Comparisons:** {}",
            result.details.total_comparisons
        );

        // Detailed breakdown
        report.push_str("## Similarity Breakdown\n\n");
        let _ = writeln!(
            report,
            "- **Token Similarity:** {:.1}%",
            result.details.token_similarity * 100.0
        );
        let _ = writeln!(
            report,
            "- **N-gram Similarity:** {:.1}%",
            result.details.ngram_similarity * 100.0
        );
        if let Some(semantic) = result.details.semantic_similarity {
            let _ = writeln!(
                report,
                "- **Semantic Similarity:** {:.1}%",
                semantic * 100.0
            );
        }
        report.push('\n');

        // Matching elements
        if !result.details.matches.is_empty() {
            report.push_str("## Matching Elements\n\n");
            let _ = writeln!(
                report,
                "Found {} matching tokens/phrases:",
                result.details.matches.len()
            );
            for (i, match_str) in result.details.matches.iter().enumerate().take(10) {
                let _ = writeln!(report, "{}. `{}`", i + 1, match_str);
            }
            if result.details.matches.len() > 10 {
                report.push('\n');
                let _ = writeln!(
                    report,
                    "*...and {} more*",
                    result.details.matches.len() - 10
                );
            }
            report.push('\n');
        }

        // Explanation
        report.push_str("## Analysis\n\n");
        report.push_str(&result.explanation);
        report.push_str("\n\n");

        // Recommendation
        report.push_str("## Recommendation\n\n");
        if result.is_plagiarized {
            if result.confidence >= 80 {
                report.push_str("**High confidence plagiarism detected.** Manual review recommended before taking action.\n");
            } else if result.confidence >= 60 {
                report.push_str("**Moderate confidence plagiarism detected.** Careful manual review required.\n");
            } else {
                report.push_str(
                    "**Low confidence plagiarism detected.** Thorough manual review necessary.\n",
                );
            }
        } else {
            report.push_str("Content appears to be original. No action needed.\n");
        }

        report
    }

    /// Generate a compact text summary
    #[must_use]
    pub fn generate_summary(result: &PlagiarismResult) -> String {
        let status = if result.is_plagiarized {
            "FLAGGED"
        } else {
            "OK"
        };
        format!(
            "[{}] Similarity: {:.1}% | Confidence: {}% | Tokens: {:.1}% | N-grams: {:.1}%",
            status,
            result.similarity_score * 100.0,
            result.confidence,
            result.details.token_similarity * 100.0,
            result.details.ngram_similarity * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_code() {
        let code = "fn hello_world() { println!(\"Hello\"); }";
        let tokens = PlagiarismDetector::tokenize_code(code);
        assert!(tokens.contains(&"fn".to_string()));
        assert!(tokens.contains(&"hello_world".to_string()));
        assert!(tokens.contains(&"println".to_string()));
    }

    #[test]
    fn test_tokenize_text() {
        let text = "Hello, world! This is a test.";
        let tokens = PlagiarismDetector::tokenize_text(text);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn test_token_similarity_identical() {
        let tokens1 = vec!["hello".to_string(), "world".to_string()];
        let tokens2 = vec!["hello".to_string(), "world".to_string()];
        let similarity = PlagiarismDetector::calculate_token_similarity(&tokens1, &tokens2);
        assert!((similarity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_token_similarity_different() {
        let tokens1 = vec!["hello".to_string(), "world".to_string()];
        let tokens2 = vec!["foo".to_string(), "bar".to_string()];
        let similarity = PlagiarismDetector::calculate_token_similarity(&tokens1, &tokens2);
        assert!((similarity - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_ngram_extraction() {
        let config = PlagiarismConfig::default();
        let detector = PlagiarismDetector::new(config);
        let ngrams = detector.extract_ngrams("hello");
        assert_eq!(ngrams.len(), 3); // "hel", "ell", "llo"
        assert!(ngrams.contains(&"hel".to_string()));
    }

    #[test]
    fn test_ngram_similarity() {
        let config = PlagiarismConfig::default();
        let detector = PlagiarismDetector::new(config);
        let similarity = detector.calculate_ngram_similarity("hello world", "hello world");
        assert!((similarity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_find_matching_tokens() {
        let tokens1 = vec!["hello".to_string(), "world".to_string(), "foo".to_string()];
        let tokens2 = vec!["hello".to_string(), "bar".to_string(), "foo".to_string()];
        let matches = PlagiarismDetector::find_matching_tokens(&tokens1, &tokens2);
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&"hello".to_string()));
        assert!(matches.contains(&"foo".to_string()));
    }

    #[test]
    fn test_similarity_matrix() {
        let mut matrix = SimilarityMatrix::new(3);
        matrix.add_score(0, 1, 0.8);
        matrix.add_score(1, 2, 0.3);
        matrix.add_score(0, 2, 0.2);

        assert_eq!(matrix.get_score(0, 1), Some(0.8));
        assert_eq!(matrix.get_score(1, 2), Some(0.3));
    }

    #[test]
    fn test_find_clusters() {
        let mut matrix = SimilarityMatrix::new(4);
        matrix.add_score(0, 1, 0.9); // 0 and 1 are similar
        matrix.add_score(2, 3, 0.85); // 2 and 3 are similar
        matrix.add_score(0, 2, 0.3); // Different clusters

        let clusters = matrix.find_clusters(0.7);
        assert_eq!(clusters.len(), 2);
    }

    #[tokio::test]
    async fn test_detect_code_plagiarism_without_llm() {
        let config = PlagiarismConfig {
            similarity_threshold: 0.7,
            use_semantic_analysis: false,
            ngram_size: 3,
            min_token_overlap: 5,
        };
        let detector = PlagiarismDetector::new(config);

        // Use more similar code for testing
        let original = "fn add(a: i32, b: i32) -> i32 { return a + b; }";
        let submission = "fn add(a: i32, b: i32) -> i32 { return a + b; }";

        let result = detector
            .detect_code_plagiarism(original, submission)
            .await
            .unwrap();
        assert!(result.similarity_score > 0.8);
        assert!(result.confidence > 0);
        assert!(result.is_plagiarized);
    }

    #[tokio::test]
    async fn test_detect_text_plagiarism_without_llm() {
        let config = PlagiarismConfig {
            similarity_threshold: 0.7,
            use_semantic_analysis: false,
            ngram_size: 3,
            min_token_overlap: 5,
        };
        let detector = PlagiarismDetector::new(config);

        let original = "The quick brown fox jumps over the lazy dog.";
        let submission = "The quick brown fox jumps over the lazy cat.";

        let result = detector
            .detect_text_plagiarism(original, submission)
            .await
            .unwrap();
        assert!(result.similarity_score > 0.5);
        assert!(result.confidence > 0);
    }

    #[test]
    fn test_plagiarism_report_markdown() {
        let result = PlagiarismResult {
            similarity_score: 0.85,
            is_plagiarized: true,
            details: SimilarityDetails {
                token_similarity: 0.80,
                ngram_similarity: 0.75,
                semantic_similarity: Some(0.90),
                matches: vec!["hello".to_string(), "world".to_string()],
                total_comparisons: 100,
            },
            explanation: "High similarity detected".to_string(),
            confidence: 85,
        };

        let report = PlagiarismReport::generate_markdown(&result, "doc1.txt", "doc2.txt");
        assert!(report.contains("# Plagiarism Detection Report"));
        assert!(report.contains("PLAGIARISM DETECTED"));
        assert!(report.contains("85.0%"));
        assert!(report.contains("doc1.txt"));
        assert!(report.contains("doc2.txt"));
    }

    #[test]
    fn test_plagiarism_report_summary() {
        let result = PlagiarismResult {
            similarity_score: 0.75,
            is_plagiarized: true,
            details: SimilarityDetails {
                token_similarity: 0.70,
                ngram_similarity: 0.65,
                semantic_similarity: None,
                matches: vec![],
                total_comparisons: 50,
            },
            explanation: "Moderate similarity".to_string(),
            confidence: 70,
        };

        let summary = PlagiarismReport::generate_summary(&result);
        assert!(summary.contains("FLAGGED"));
        assert!(summary.contains("75.0%"));
        assert!(summary.contains("70%"));
    }
}
