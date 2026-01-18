//! GitHub API integration for code verification
//!
//! Provides functionality to:
//! - Fetch code from repositories
//! - Analyze commit diffs
//! - Verify releases, commits, and PRs
//! - Automate PR reviews

use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::error::{AiError, Result};

/// GitHub API client
#[derive(Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
    config: GitHubConfig,
}

/// GitHub client configuration
#[derive(Debug, Clone)]
pub struct GitHubConfig {
    /// Personal access token for authentication
    pub token: Option<String>,
    /// API base URL (default: <https://api.github.com>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            token: None,
            api_base: "https://api.github.com".to_string(),
            timeout_secs: 30,
        }
    }
}

impl GitHubConfig {
    /// Create from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            token: std::env::var("GITHUB_TOKEN").ok(),
            ..Default::default()
        }
    }

    /// Set token
    #[must_use]
    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }
}

impl GitHubClient {
    /// Create a new GitHub client
    pub fn new(config: GitHubConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.v3+json"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("kaccy-ai/1.0"));

        if let Some(ref token) = config.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|e| AiError::GitHub(format!("Invalid token: {e}")))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| AiError::GitHub(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { client, config })
    }

    /// Build full API URL
    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.config.api_base, path)
    }

    /// Get repository information
    pub async fn get_repository(&self, owner: &str, repo: &str) -> Result<Repository> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get file contents from a repository
    pub async fn get_file_contents(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_name: Option<&str>,
    ) -> Result<FileContents> {
        let mut url = self.api_url(&format!("/repos/{owner}/{repo}/contents/{path}"));
        if let Some(r) = ref_name {
            url = format!("{url}?ref={r}");
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get a specific commit
    pub async fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<Commit> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/commits/{sha}"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get commit diff
    pub async fn get_commit_diff(&self, owner: &str, repo: &str, sha: &str) -> Result<String> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/commits/{sha}"));
        let response = self
            .client
            .get(&url)
            .header(ACCEPT, "application/vnd.github.diff")
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to get diff: {e}")))
    }

    /// Get pull request details
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PullRequest> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/pulls/{pr_number}"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get pull request diff
    pub async fn get_pull_request_diff(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<String> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/pulls/{pr_number}"));
        let response = self
            .client
            .get(&url)
            .header(ACCEPT, "application/vnd.github.diff")
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to get diff: {e}")))
    }

    /// Get pull request files
    pub async fn get_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PullRequestFile>> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/pulls/{pr_number}/files"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get a release by tag
    pub async fn get_release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> Result<Release> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/releases/tags/{tag}"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get latest release
    pub async fn get_latest_release(&self, owner: &str, repo: &str) -> Result<Release> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/releases/latest"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Get issue details
    pub async fn get_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<Issue> {
        let url = self.api_url(&format!("/repos/{owner}/{repo}/issues/{issue_number}"));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// List recent commits
    pub async fn list_commits(
        &self,
        owner: &str,
        repo: &str,
        since: Option<&str>,
        per_page: Option<u32>,
    ) -> Result<Vec<CommitSummary>> {
        let mut url = self.api_url(&format!("/repos/{owner}/{repo}/commits"));
        let mut params = Vec::new();
        if let Some(s) = since {
            params.push(format!("since={s}"));
        }
        if let Some(p) = per_page {
            params.push(format!("per_page={p}"));
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }

    /// Search code in repositories
    pub async fn search_code(
        &self,
        query: &str,
        per_page: Option<u32>,
    ) -> Result<CodeSearchResult> {
        let mut url = self.api_url(&format!("/search/code?q={}", urlencoding::encode(query)));
        if let Some(p) = per_page {
            url = format!("{url}&per_page={p}");
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::GitHub(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::GitHub(format!(
                "GitHub API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AiError::GitHub(format!("Failed to parse response: {e}")))
    }
}

// ============== Data Types ==============

/// Repository information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: String,
    pub stargazers_count: u32,
    pub forks_count: u32,
    pub language: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub pushed_at: Option<String>,
}

/// File contents from repository
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileContents {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub url: String,
    pub html_url: String,
    pub download_url: Option<String>,
    pub content: Option<String>,
    pub encoding: Option<String>,
    #[serde(rename = "type")]
    pub file_type: String,
}

impl FileContents {
    /// Decode base64 content
    pub fn decode_content(&self) -> Result<String> {
        if let Some(ref content) = self.content {
            // Remove newlines from base64 content
            let clean_content: String = content.chars().filter(|c| !c.is_whitespace()).collect();
            let decoded = base64_decode(&clean_content)?;
            String::from_utf8(decoded)
                .map_err(|e| AiError::GitHub(format!("Invalid UTF-8 content: {e}")))
        } else {
            Err(AiError::GitHub("No content available".to_string()))
        }
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    // Simple base64 decoder
    const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for c in input.bytes() {
        if c == b'=' {
            break;
        }

        let value =
            BASE64_CHARS.iter().position(|&x| x == c).ok_or_else(|| {
                AiError::GitHub(format!("Invalid base64 character: {}", c as char))
            })? as u32;

        buffer = (buffer << 6) | value;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

/// Commit information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Commit {
    pub sha: String,
    pub html_url: String,
    pub commit: CommitData,
    pub author: Option<GitHubUser>,
    pub committer: Option<GitHubUser>,
    pub stats: Option<CommitStats>,
    pub files: Option<Vec<CommitFile>>,
}

/// Commit summary (from list endpoint)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub html_url: String,
    pub commit: CommitData,
    pub author: Option<GitHubUser>,
    pub committer: Option<GitHubUser>,
}

/// Commit data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitData {
    pub message: String,
    pub author: GitAuthor,
    pub committer: GitAuthor,
}

/// Git author
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitAuthor {
    pub name: String,
    pub email: String,
    pub date: String,
}

/// Commit statistics
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitStats {
    pub additions: u32,
    pub deletions: u32,
    pub total: u32,
}

/// File changed in a commit
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitFile {
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
}

/// GitHub user
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    pub html_url: String,
}

/// Pull request information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequest {
    pub id: u64,
    pub number: u64,
    pub state: String,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub user: GitHubUser,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merge_commit_sha: Option<String>,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub changed_files: Option<u32>,
}

impl PullRequest {
    #[must_use]
    pub fn is_merged(&self) -> bool {
        self.merged_at.is_some()
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == "closed"
    }
}

/// Pull request reference (head/base)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestRef {
    pub label: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

/// File in a pull request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestFile {
    pub sha: String,
    pub filename: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
    pub raw_url: String,
}

/// Release information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
    pub created_at: String,
    pub published_at: Option<String>,
    pub author: GitHubUser,
    pub assets: Vec<ReleaseAsset>,
}

/// Release asset
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub download_count: u32,
    pub browser_download_url: String,
}

/// Issue information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Issue {
    pub id: u64,
    pub number: u64,
    pub state: String,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub user: GitHubUser,
    pub labels: Vec<IssueLabel>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

impl Issue {
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == "closed"
    }
}

/// Issue label
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueLabel {
    pub name: String,
    pub color: String,
}

/// Code search result
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeSearchResult {
    pub total_count: u32,
    pub incomplete_results: bool,
    pub items: Vec<CodeSearchItem>,
}

/// Code search item
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeSearchItem {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub html_url: String,
    pub repository: CodeSearchRepository,
}

/// Repository in code search
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeSearchRepository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
}

// ============== Verification Helpers ==============

/// GitHub verification service
#[derive(Clone)]
pub struct GitHubVerifier {
    client: GitHubClient,
}

impl GitHubVerifier {
    #[must_use]
    pub fn new(client: GitHubClient) -> Self {
        Self { client }
    }

    /// Verify a commit exists and get details
    pub async fn verify_commit(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<CommitVerification> {
        let commit = self.client.get_commit(owner, repo, sha).await?;

        Ok(CommitVerification {
            exists: true,
            sha: commit.sha,
            message: commit.commit.message,
            author: commit.commit.author.name,
            date: commit.commit.author.date,
            stats: commit.stats,
            url: commit.html_url,
        })
    }

    /// Verify a release exists
    pub async fn verify_release(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
    ) -> Result<ReleaseVerification> {
        let release = self.client.get_release_by_tag(owner, repo, tag).await?;

        Ok(ReleaseVerification {
            exists: true,
            tag: release.tag_name,
            name: release.name,
            body: release.body,
            is_prerelease: release.prerelease,
            is_draft: release.draft,
            published_at: release.published_at,
            assets_count: release.assets.len(),
            url: release.html_url,
        })
    }

    /// Verify a PR is merged
    pub async fn verify_pr_merged(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrVerification> {
        let pr = self.client.get_pull_request(owner, repo, pr_number).await?;
        let is_merged = pr.is_merged();

        Ok(PrVerification {
            exists: true,
            number: pr.number,
            title: pr.title,
            state: pr.state,
            is_merged,
            merged_at: pr.merged_at,
            author: pr.user.login,
            additions: pr.additions,
            deletions: pr.deletions,
            url: pr.html_url,
        })
    }

    /// Verify an issue is closed
    pub async fn verify_issue_closed(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<IssueVerification> {
        let issue = self.client.get_issue(owner, repo, issue_number).await?;
        let is_closed = issue.is_closed();

        Ok(IssueVerification {
            exists: true,
            number: issue.number,
            title: issue.title,
            state: issue.state,
            is_closed,
            closed_at: issue.closed_at,
            author: issue.user.login,
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
            url: issue.html_url,
        })
    }

    /// Parse a GitHub URL and verify the resource
    pub async fn verify_url(&self, url: &str) -> Result<GitHubVerificationResult> {
        let parsed = parse_github_url(url)?;

        match parsed {
            ParsedGitHubUrl::Commit { owner, repo, sha } => {
                let verification = self.verify_commit(&owner, &repo, &sha).await?;
                Ok(GitHubVerificationResult::Commit(verification))
            }
            ParsedGitHubUrl::PullRequest {
                owner,
                repo,
                number,
            } => {
                let verification = self.verify_pr_merged(&owner, &repo, number).await?;
                Ok(GitHubVerificationResult::PullRequest(verification))
            }
            ParsedGitHubUrl::Release { owner, repo, tag } => {
                let verification = self.verify_release(&owner, &repo, &tag).await?;
                Ok(GitHubVerificationResult::Release(verification))
            }
            ParsedGitHubUrl::Issue {
                owner,
                repo,
                number,
            } => {
                let verification = self.verify_issue_closed(&owner, &repo, number).await?;
                Ok(GitHubVerificationResult::Issue(verification))
            }
            ParsedGitHubUrl::Repository { owner, repo } => {
                let repository = self.client.get_repository(&owner, &repo).await?;
                Ok(GitHubVerificationResult::Repository(repository))
            }
        }
    }
}

/// Parsed GitHub URL
#[derive(Debug, Clone)]
pub enum ParsedGitHubUrl {
    Commit {
        owner: String,
        repo: String,
        sha: String,
    },
    PullRequest {
        owner: String,
        repo: String,
        number: u64,
    },
    Release {
        owner: String,
        repo: String,
        tag: String,
    },
    Issue {
        owner: String,
        repo: String,
        number: u64,
    },
    Repository {
        owner: String,
        repo: String,
    },
}

/// Parse a GitHub URL into its components
pub fn parse_github_url(url: &str) -> Result<ParsedGitHubUrl> {
    // Remove trailing slash and normalize
    let url = url.trim_end_matches('/');

    // Check for github.com
    if !url.contains("github.com") {
        return Err(AiError::GitHub("Not a GitHub URL".to_string()));
    }

    // Extract path parts
    let path = url
        .split("github.com/")
        .nth(1)
        .ok_or_else(|| AiError::GitHub("Invalid GitHub URL format".to_string()))?;

    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 2 {
        return Err(AiError::GitHub(
            "Invalid GitHub URL: missing owner/repo".to_string(),
        ));
    }

    let owner = parts[0].to_string();
    let repo = parts[1].to_string();

    // Parse specific resource type
    if parts.len() >= 4 {
        match parts[2] {
            "commit" | "commits" => {
                let sha = parts[3].to_string();
                return Ok(ParsedGitHubUrl::Commit { owner, repo, sha });
            }
            "pull" => {
                let number = parts[3]
                    .parse()
                    .map_err(|_| AiError::GitHub("Invalid PR number".to_string()))?;
                return Ok(ParsedGitHubUrl::PullRequest {
                    owner,
                    repo,
                    number,
                });
            }
            "releases" if parts.len() >= 5 && parts[3] == "tag" => {
                let tag = parts[4].to_string();
                return Ok(ParsedGitHubUrl::Release { owner, repo, tag });
            }
            "issues" => {
                let number = parts[3]
                    .parse()
                    .map_err(|_| AiError::GitHub("Invalid issue number".to_string()))?;
                return Ok(ParsedGitHubUrl::Issue {
                    owner,
                    repo,
                    number,
                });
            }
            _ => {}
        }
    }

    // Default to repository
    Ok(ParsedGitHubUrl::Repository { owner, repo })
}

// ============== Verification Results ==============

/// Commit verification result
#[derive(Debug, Clone, Serialize)]
pub struct CommitVerification {
    pub exists: bool,
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub stats: Option<CommitStats>,
    pub url: String,
}

/// Release verification result
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseVerification {
    pub exists: bool,
    pub tag: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub is_prerelease: bool,
    pub is_draft: bool,
    pub published_at: Option<String>,
    pub assets_count: usize,
    pub url: String,
}

/// PR verification result
#[derive(Debug, Clone, Serialize)]
pub struct PrVerification {
    pub exists: bool,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_merged: bool,
    pub merged_at: Option<String>,
    pub author: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub url: String,
}

/// Issue verification result
#[derive(Debug, Clone, Serialize)]
pub struct IssueVerification {
    pub exists: bool,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_closed: bool,
    pub closed_at: Option<String>,
    pub author: String,
    pub labels: Vec<String>,
    pub url: String,
}

/// GitHub verification result enum
#[derive(Debug, Clone, Serialize)]
pub enum GitHubVerificationResult {
    Commit(CommitVerification),
    PullRequest(PrVerification),
    Release(ReleaseVerification),
    Issue(IssueVerification),
    Repository(Repository),
}
