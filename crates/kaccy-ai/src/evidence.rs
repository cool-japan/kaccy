//! Evidence parsing and validation for commitment verification
//!
//! Extracts URLs, validates evidence format, and structures data for admin review.
//!
//! # Examples
//!
//! ```
//! use kaccy_ai::EvidenceParser;
//!
//! let parser = EvidenceParser::new();
//! let evidence_url = "https://github.com/owner/repo/releases/tag/v1.0.0";
//! let description = "Released version 1.0.0 with new features. See https://example.com/docs";
//!
//! match parser.parse(evidence_url, description) {
//!     Ok(parsed) => {
//!         println!("Evidence type: {:?}", parsed.evidence_type);
//!         println!("Valid URL: {}", parsed.url_valid);
//!         println!("Additional URLs: {:?}", parsed.additional_urls);
//!     }
//!     Err(e) => eprintln!("Parse error: {}", e),
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

use crate::error::{AiError, Result};

/// Parsed evidence from user submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEvidence {
    /// Original evidence URL
    pub primary_url: String,
    /// Additional URLs extracted from description
    pub additional_urls: Vec<String>,
    /// Type of evidence detected
    pub evidence_type: EvidenceType,
    /// Whether the URL is valid and accessible
    pub url_valid: bool,
    /// Structured data extracted from the evidence
    pub structured_data: Option<StructuredEvidence>,
    /// Suggested verification decision for admin
    pub suggested_decision: SuggestedDecision,
}

/// Type of evidence detected from URL patterns
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvidenceType {
    /// GitHub repository, commit, release, or PR
    GitHub,
    /// `YouTube` video
    YouTube,
    /// Twitter/X post
    Twitter,
    /// Generic website URL
    Website,
    /// Blog post (Medium, Substack, etc.)
    BlogPost,
    /// Documentation site
    Documentation,
    /// Image (PNG, JPG, etc.)
    Image,
    /// PDF document
    Pdf,
    /// Unknown type
    Unknown,
}

/// Structured evidence data extracted from different sources
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StructuredEvidence {
    /// GitHub-specific evidence
    GitHub {
        /// Repository owner login.
        owner: String,
        /// Repository name.
        repo: String,
        /// The specific GitHub resource kind.
        evidence_kind: GitHubEvidenceKind,
    },
    /// `YouTube` video evidence
    YouTube {
        /// YouTube video identifier.
        video_id: String,
    },
    /// Twitter/X post evidence
    Twitter {
        /// Tweet snowflake ID.
        tweet_id: String,
        /// Twitter username.
        username: String,
    },
    /// Generic URL evidence
    Generic {
        /// Domain of the URL.
        domain: String,
    },
}

/// Kind of GitHub evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitHubEvidenceKind {
    /// A repository link with no specific resource.
    Repository,
    /// A specific commit.
    Commit {
        /// Commit SHA hash.
        hash: String,
    },
    /// A release.
    Release {
        /// Release tag name.
        tag: String,
    },
    /// A pull request.
    PullRequest {
        /// Pull request number.
        number: u64,
    },
    /// An issue.
    Issue {
        /// Issue number.
        number: u64,
    },
}

/// Suggested verification decision for admin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedDecision {
    /// Recommended action
    pub recommendation: Recommendation,
    /// Confidence in the recommendation (0.0-1.0)
    pub confidence: f64,
    /// Reason for the recommendation
    pub reason: String,
}

/// Recommended action for admin
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Recommendation {
    /// Approve the commitment
    Approve,
    /// Reject the commitment
    Reject,
    /// Need more investigation
    NeedsReview,
}

/// Evidence parser service
pub struct EvidenceParser;

impl Default for EvidenceParser {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceParser {
    /// Create a new `EvidenceParser`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse evidence from URL and description
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_ai::EvidenceParser;
    ///
    /// let parser = EvidenceParser::new();
    /// let result = parser.parse(
    ///     "https://github.com/owner/repo",
    ///     "My project repository"
    /// );
    /// assert!(result.is_ok());
    /// ```
    pub fn parse(&self, evidence_url: &str, description: &str) -> Result<ParsedEvidence> {
        // Validate primary URL
        let url_valid = Self::validate_url(evidence_url);
        if !url_valid {
            return Err(AiError::Validation(format!(
                "Invalid evidence URL: {evidence_url}"
            )));
        }

        // Detect evidence type
        let evidence_type = Self::detect_evidence_type(evidence_url);

        // Extract additional URLs from description
        let additional_urls = Self::extract_urls(description)
            .into_iter()
            .filter(|u| u != evidence_url)
            .collect();

        // Parse structured data from URL
        let structured_data = Self::parse_structured_evidence(evidence_url, evidence_type);

        // Generate suggested decision
        let suggested_decision = Self::generate_suggestion(&evidence_type, &structured_data);

        Ok(ParsedEvidence {
            primary_url: evidence_url.to_string(),
            additional_urls,
            evidence_type,
            url_valid,
            structured_data,
            suggested_decision,
        })
    }

    /// Validate URL format
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_ai::EvidenceParser;
    ///
    /// assert!(EvidenceParser::validate_url("https://github.com/owner/repo"));
    /// assert!(!EvidenceParser::validate_url("http://github.com/owner/repo")); // Must be HTTPS
    /// assert!(!EvidenceParser::validate_url("https://bit.ly/short")); // Blocked domain
    /// ```
    #[must_use]
    pub fn validate_url(url: &str) -> bool {
        // Must be HTTPS
        if !url.starts_with("https://") {
            return false;
        }

        // Basic URL structure validation
        let url_without_scheme = &url[8..];
        if url_without_scheme.is_empty() {
            return false;
        }

        // Must have a domain
        let parts: Vec<&str> = url_without_scheme.splitn(2, '/').collect();
        if parts.is_empty() {
            return false;
        }

        let domain = parts[0];
        if domain.is_empty() || !domain.contains('.') {
            return false;
        }

        // Check for blocked domains (common spam/phishing)
        let blocked_domains = ["bit.ly", "tinyurl.com", "t.co", "goo.gl", "ow.ly"];
        let domain_lower = domain.to_lowercase();
        if blocked_domains.iter().any(|b| domain_lower == *b) {
            return false;
        }

        true
    }

    /// Detect evidence type from URL patterns
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_ai::{EvidenceParser, EvidenceType};
    ///
    /// assert_eq!(
    ///     EvidenceParser::detect_evidence_type("https://github.com/owner/repo"),
    ///     EvidenceType::GitHub
    /// );
    /// assert_eq!(
    ///     EvidenceParser::detect_evidence_type("https://youtube.com/watch?v=abc"),
    ///     EvidenceType::YouTube
    /// );
    /// assert_eq!(
    ///     EvidenceParser::detect_evidence_type("https://twitter.com/user/status/123"),
    ///     EvidenceType::Twitter
    /// );
    /// ```
    #[must_use]
    pub fn detect_evidence_type(url: &str) -> EvidenceType {
        let url_lower = url.to_lowercase();

        if url_lower.contains("github.com") {
            EvidenceType::GitHub
        } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
            EvidenceType::YouTube
        } else if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
            EvidenceType::Twitter
        } else if url_lower.contains("medium.com") || url_lower.contains("substack.com") {
            EvidenceType::BlogPost
        } else if url_lower.ends_with(".pdf") {
            EvidenceType::Pdf
        } else if url_lower.ends_with(".png")
            || url_lower.ends_with(".jpg")
            || url_lower.ends_with(".jpeg")
            || url_lower.ends_with(".gif")
            || url_lower.ends_with(".webp")
        {
            EvidenceType::Image
        } else if url_lower.contains("docs.") || url_lower.contains("/docs/") {
            EvidenceType::Documentation
        } else {
            EvidenceType::Website
        }
    }

    /// Extract URLs from text
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_ai::EvidenceParser;
    ///
    /// let text = "Check out https://github.com/owner/repo and https://example.com/docs";
    /// let urls = EvidenceParser::extract_urls(text);
    /// assert_eq!(urls.len(), 2);
    /// assert!(urls.contains(&"https://github.com/owner/repo".to_string()));
    /// ```
    #[must_use]
    pub fn extract_urls(text: &str) -> Vec<String> {
        let mut urls = Vec::new();

        // Simple URL extraction - find https:// and collect until whitespace
        for part in text.split_whitespace() {
            if part.starts_with("https://") {
                // Clean up trailing punctuation
                let clean = part.trim_end_matches(['.', ',', ')', ']']);
                if Self::validate_url(clean) {
                    urls.push(clean.to_string());
                }
            }
        }

        urls
    }

    /// Parse structured evidence from URL
    fn parse_structured_evidence(
        url: &str,
        evidence_type: EvidenceType,
    ) -> Option<StructuredEvidence> {
        match evidence_type {
            EvidenceType::GitHub => Self::parse_github_url(url),
            EvidenceType::YouTube => Self::parse_youtube_url(url),
            EvidenceType::Twitter => Self::parse_twitter_url(url),
            _ => {
                // Extract domain for generic URLs
                let domain = url
                    .strip_prefix("https://")
                    .and_then(|u| u.split('/').next())
                    .map(std::string::ToString::to_string)?;
                Some(StructuredEvidence::Generic { domain })
            }
        }
    }

    /// Parse GitHub URL into structured data
    fn parse_github_url(url: &str) -> Option<StructuredEvidence> {
        let path = url.strip_prefix("https://github.com/")?;
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() < 2 {
            return None;
        }

        let owner = parts[0].to_string();
        let repo = parts[1].to_string();

        let evidence_kind = if parts.len() >= 4 {
            match parts[2] {
                "commit" => GitHubEvidenceKind::Commit {
                    hash: parts[3].to_string(),
                },
                "releases" | "release" => GitHubEvidenceKind::Release {
                    tag: (*parts.get(4).unwrap_or(&"latest")).to_string(),
                },
                "pull" => GitHubEvidenceKind::PullRequest {
                    number: parts[3].parse().ok()?,
                },
                "issues" => GitHubEvidenceKind::Issue {
                    number: parts[3].parse().ok()?,
                },
                _ => GitHubEvidenceKind::Repository,
            }
        } else {
            GitHubEvidenceKind::Repository
        };

        Some(StructuredEvidence::GitHub {
            owner,
            repo,
            evidence_kind,
        })
    }

    /// Parse `YouTube` URL into structured data
    fn parse_youtube_url(url: &str) -> Option<StructuredEvidence> {
        // Handle youtube.com/watch?v=VIDEO_ID
        if url.contains("youtube.com/watch") {
            let video_id = url
                .split("v=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .map(std::string::ToString::to_string)?;
            return Some(StructuredEvidence::YouTube { video_id });
        }

        // Handle youtu.be/VIDEO_ID
        if url.contains("youtu.be/") {
            let video_id = url
                .split("youtu.be/")
                .nth(1)
                .and_then(|s| s.split('?').next())
                .map(std::string::ToString::to_string)?;
            return Some(StructuredEvidence::YouTube { video_id });
        }

        None
    }

    /// Parse Twitter/X URL into structured data
    fn parse_twitter_url(url: &str) -> Option<StructuredEvidence> {
        let path = url
            .strip_prefix("https://twitter.com/")
            .or_else(|| url.strip_prefix("https://x.com/"))?;

        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 3 && parts[1] == "status" {
            return Some(StructuredEvidence::Twitter {
                username: parts[0].to_string(),
                tweet_id: parts[2].to_string(),
            });
        }

        None
    }

    /// Generate suggested verification decision
    fn generate_suggestion(
        evidence_type: &EvidenceType,
        structured_data: &Option<StructuredEvidence>,
    ) -> SuggestedDecision {
        match (evidence_type, structured_data) {
            // GitHub commits and releases are strong evidence
            (
                EvidenceType::GitHub,
                Some(StructuredEvidence::GitHub {
                    evidence_kind: GitHubEvidenceKind::Commit { .. },
                    ..
                }),
            ) => SuggestedDecision {
                recommendation: Recommendation::NeedsReview,
                confidence: 0.7,
                reason: "GitHub commit detected. Verify commit matches commitment description."
                    .to_string(),
            },
            (
                EvidenceType::GitHub,
                Some(StructuredEvidence::GitHub {
                    evidence_kind: GitHubEvidenceKind::Release { .. },
                    ..
                }),
            ) => SuggestedDecision {
                recommendation: Recommendation::NeedsReview,
                confidence: 0.8,
                reason: "GitHub release detected. Strong evidence if release matches commitment."
                    .to_string(),
            },
            // YouTube videos need manual review
            (EvidenceType::YouTube, Some(StructuredEvidence::YouTube { .. })) => {
                SuggestedDecision {
                    recommendation: Recommendation::NeedsReview,
                    confidence: 0.5,
                    reason: "YouTube video detected. Review video content matches commitment."
                        .to_string(),
                }
            }
            // Weak evidence types
            (EvidenceType::Image, _) => SuggestedDecision {
                recommendation: Recommendation::NeedsReview,
                confidence: 0.3,
                reason: "Image evidence is weak. Consider requesting additional proof.".to_string(),
            },
            // Unknown or generic URLs
            _ => SuggestedDecision {
                recommendation: Recommendation::NeedsReview,
                confidence: 0.4,
                reason: "Manual review required to verify evidence authenticity.".to_string(),
            },
        }
    }
}

/// Generate admin verification prompt
#[must_use]
pub fn generate_verification_prompt(
    commitment_title: &str,
    commitment_description: &str,
    evidence: &ParsedEvidence,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("## Commitment Verification Request\n\n");
    let _ = writeln!(prompt, "**Title:** {commitment_title}");
    let _ = writeln!(prompt, "**Description:** {commitment_description}");
    prompt.push_str("---\n\n");

    prompt.push_str("## Evidence Submitted\n\n");
    let _ = writeln!(prompt, "**Primary URL:** {}", evidence.primary_url);
    let _ = write!(
        prompt,
        "**Evidence Type:** {:?}\n\n",
        evidence.evidence_type
    );

    if let Some(ref structured) = evidence.structured_data {
        prompt.push_str("### Parsed Evidence Data\n\n");
        match structured {
            StructuredEvidence::GitHub {
                owner,
                repo,
                evidence_kind,
            } => {
                let _ = writeln!(prompt, "- **Repository:** {owner}/{repo}");
                match evidence_kind {
                    GitHubEvidenceKind::Commit { hash } => {
                        let _ = writeln!(prompt, "- **Commit:** {hash}");
                    }
                    GitHubEvidenceKind::Release { tag } => {
                        let _ = writeln!(prompt, "- **Release:** {tag}");
                    }
                    GitHubEvidenceKind::PullRequest { number } => {
                        let _ = writeln!(prompt, "- **PR:** #{number}");
                    }
                    GitHubEvidenceKind::Issue { number } => {
                        let _ = writeln!(prompt, "- **Issue:** #{number}");
                    }
                    GitHubEvidenceKind::Repository => {
                        prompt.push_str("- **Type:** Repository link\n");
                    }
                }
            }
            StructuredEvidence::YouTube { video_id } => {
                let _ = writeln!(prompt, "- **Video ID:** {video_id}");
            }
            StructuredEvidence::Twitter { username, tweet_id } => {
                let _ = writeln!(prompt, "- **User:** @{username}");
                let _ = writeln!(prompt, "- **Tweet ID:** {tweet_id}");
            }
            StructuredEvidence::Generic { domain } => {
                let _ = writeln!(prompt, "- **Domain:** {domain}");
            }
        }
    }

    if !evidence.additional_urls.is_empty() {
        prompt.push_str("\n### Additional URLs\n\n");
        for url in &evidence.additional_urls {
            let _ = writeln!(prompt, "- {url}");
        }
    }

    prompt.push_str("\n---\n\n");
    prompt.push_str("## Suggested Decision\n\n");
    let _ = writeln!(
        prompt,
        "**Recommendation:** {:?}",
        evidence.suggested_decision.recommendation
    );
    let _ = writeln!(
        prompt,
        "**Confidence:** {:.0}%",
        evidence.suggested_decision.confidence * 100.0
    );
    let _ = writeln!(prompt, "**Reason:** {}", evidence.suggested_decision.reason);

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url() {
        assert!(EvidenceParser::validate_url("https://github.com/user/repo"));
        assert!(EvidenceParser::validate_url(
            "https://example.com/path/to/file"
        ));
        assert!(!EvidenceParser::validate_url("http://example.com")); // Not HTTPS
        assert!(!EvidenceParser::validate_url("https://")); // No domain
        assert!(!EvidenceParser::validate_url("https://bit.ly/abc")); // Blocked shortener
    }

    #[test]
    fn test_detect_evidence_type() {
        assert_eq!(
            EvidenceParser::detect_evidence_type("https://github.com/user/repo"),
            EvidenceType::GitHub
        );
        assert_eq!(
            EvidenceParser::detect_evidence_type("https://youtube.com/watch?v=abc"),
            EvidenceType::YouTube
        );
        assert_eq!(
            EvidenceParser::detect_evidence_type("https://twitter.com/user/status/123"),
            EvidenceType::Twitter
        );
        assert_eq!(
            EvidenceParser::detect_evidence_type("https://example.com/file.pdf"),
            EvidenceType::Pdf
        );
    }

    #[test]
    fn test_extract_urls() {
        let text = "Check out https://github.com/user/repo and also https://example.com/doc";
        let urls = EvidenceParser::extract_urls(text);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://github.com/user/repo".to_string()));
        assert!(urls.contains(&"https://example.com/doc".to_string()));
    }

    #[test]
    fn test_parse_github_url() {
        let evidence =
            EvidenceParser::parse_github_url("https://github.com/rust-lang/rust/commit/abc123");
        assert!(evidence.is_some());
        if let Some(StructuredEvidence::GitHub {
            owner,
            repo,
            evidence_kind,
        }) = evidence
        {
            assert_eq!(owner, "rust-lang");
            assert_eq!(repo, "rust");
            assert!(matches!(evidence_kind, GitHubEvidenceKind::Commit { .. }));
        }
    }
}
