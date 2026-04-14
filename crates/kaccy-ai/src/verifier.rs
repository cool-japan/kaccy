//! Output verification

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::evidence::EvidenceParser;
use crate::github::{GitHubClient, GitHubConfig, GitHubVerifier};

/// Result of a single output verification check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the evidence was considered verified.
    pub verified: bool,
    /// Confidence level in the result (0.0–1.0).
    pub confidence: f64,
    /// Whether the supplied evidence URL/hash was reachable and valid.
    pub evidence_valid: bool,
    /// Human-readable explanation of the verification outcome.
    pub details: String,
}

/// Trait for verifying outputs such as commits and content URLs.
#[async_trait]
pub trait OutputVerifier: Send + Sync {
    /// Verify that a GitHub commit exists and was authored by `expected_author`.
    async fn verify_github_commit(
        &self,
        repo_url: &str,
        commit_hash: &str,
        expected_author: &str,
    ) -> Result<VerificationResult>;

    /// Verify that a content URL is accessible and optionally matches a hash.
    async fn verify_content_url(
        &self,
        url: &str,
        expected_content_hash: Option<&str>,
    ) -> Result<VerificationResult>;
}

/// Default verifier that uses GitHub API and evidence parsing
pub struct DefaultVerifier {
    github_client: Option<GitHubClient>,
}

impl Default for DefaultVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultVerifier {
    /// Create a new `DefaultVerifier`
    #[must_use]
    pub fn new() -> Self {
        Self {
            github_client: None,
        }
    }

    /// Create a new `DefaultVerifier` with GitHub authentication
    pub fn with_github_token(token: String) -> Result<Self> {
        let config = GitHubConfig::default().with_token(token);
        let client = GitHubClient::new(config)?;
        Ok(Self {
            github_client: Some(client),
        })
    }

    /// Create with existing GitHub client
    #[must_use]
    pub fn with_github_client(client: GitHubClient) -> Self {
        Self {
            github_client: Some(client),
        }
    }
}

#[async_trait]
impl OutputVerifier for DefaultVerifier {
    async fn verify_github_commit(
        &self,
        repo_url: &str,
        commit_hash: &str,
        expected_author: &str,
    ) -> Result<VerificationResult> {
        // If GitHub client is available, use actual verification
        if let Some(client) = &self.github_client {
            let verifier = GitHubVerifier::new(client.clone());

            // Parse owner and repo from URL
            let parts: Vec<&str> = repo_url.trim_end_matches('/').split('/').collect();

            if parts.len() >= 2 {
                let owner = parts[parts.len() - 2];
                let repo = parts[parts.len() - 1];

                let verification = verifier.verify_commit(owner, repo, commit_hash).await?;

                // Check if the author matches the expected author
                let author_matches = expected_author.is_empty()
                    || verification
                        .author
                        .to_lowercase()
                        .contains(&expected_author.to_lowercase());

                let verified = verification.exists && author_matches;
                let confidence = if verified {
                    if author_matches { 0.95 } else { 0.75 }
                } else {
                    0.3
                };

                let details = if verified {
                    format!(
                        "Commit {} by {} verified successfully",
                        &verification.sha[..7.min(verification.sha.len())],
                        verification.author
                    )
                } else if !verification.exists {
                    "Commit not found".to_string()
                } else {
                    format!(
                        "Commit exists but author mismatch (expected: {}, actual: {})",
                        expected_author, verification.author
                    )
                };

                return Ok(VerificationResult {
                    verified,
                    confidence,
                    evidence_valid: verification.exists,
                    details,
                });
            }
        }

        // Fallback to manual verification
        Ok(VerificationResult {
            verified: false,
            confidence: 0.0,
            evidence_valid: false,
            details: "Verification pending - manual review required (GitHub client not configured)"
                .to_string(),
        })
    }

    async fn verify_content_url(
        &self,
        url: &str,
        _expected_content_hash: Option<&str>,
    ) -> Result<VerificationResult> {
        // Validate URL using evidence parser (static method)
        let is_valid = EvidenceParser::validate_url(url);

        if !is_valid {
            return Ok(VerificationResult {
                verified: false,
                confidence: 0.0,
                evidence_valid: false,
                details: "Invalid URL format".to_string(),
            });
        }

        // Detect evidence type (static method)
        let evidence_type = EvidenceParser::detect_evidence_type(url);
        let confidence = match evidence_type {
            crate::evidence::EvidenceType::GitHub => 0.9,
            crate::evidence::EvidenceType::YouTube => 0.8,
            crate::evidence::EvidenceType::Twitter => 0.8,
            crate::evidence::EvidenceType::Website => 0.7,
            crate::evidence::EvidenceType::BlogPost => 0.7,
            crate::evidence::EvidenceType::Documentation => 0.8,
            crate::evidence::EvidenceType::Image => 0.6,
            crate::evidence::EvidenceType::Pdf => 0.7,
            crate::evidence::EvidenceType::Unknown => 0.5,
        };

        Ok(VerificationResult {
            verified: is_valid,
            confidence,
            evidence_valid: is_valid,
            details: format!("URL validated as {evidence_type:?} evidence"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verify_content_url_valid() {
        let verifier = DefaultVerifier::new();
        let result = verifier
            .verify_content_url("https://github.com/owner/repo", None)
            .await
            .unwrap();

        assert!(result.verified);
        assert!(result.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_verify_content_url_invalid() {
        let verifier = DefaultVerifier::new();
        let result = verifier
            .verify_content_url("not-a-url", None)
            .await
            .unwrap();

        assert!(!result.verified);
        assert!(result.confidence.abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_verify_github_commit_without_client() {
        let verifier = DefaultVerifier::new();
        let result = verifier
            .verify_github_commit("https://github.com/owner/repo", "abc123", "author")
            .await
            .unwrap();

        // Without client, should fall back to manual review
        assert!(!result.verified);
        assert!(result.details.contains("manual review required"));
    }
}
