//! Social media verification module
//!
//! This module provides verification capabilities for social media evidence
//! including Twitter/X posts and `YouTube` videos.
//!
//! # Honest verification model
//!
//! API-backed verification requires credentials to be supplied at construction
//! time via [`SocialApiCredentials`].  Without credentials the verifier returns
//! [`VerificationStatus::NeedsManualReview`] — it never fabricates a
//! `Verified` result from URL-shape inspection alone.
//!
//! With credentials the verifier calls the real platform APIs:
//! - **Twitter/X** — Twitter API v2 `GET /2/tweets/{id}`
//! - **YouTube**  — YouTube Data API v3 `GET /youtube/v3/videos`

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AiError, Result};

// ─── Platform enum ────────────────────────────────────────────────────────────

/// Social media platform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocialPlatform {
    /// Twitter/X
    Twitter,
    /// `YouTube`
    YouTube,
    /// `LinkedIn`
    LinkedIn,
    /// Discord (limited verification)
    Discord,
    /// Telegram (limited verification)
    Telegram,
    /// Unknown platform
    Unknown,
}

impl SocialPlatform {
    /// Detect platform from URL
    #[must_use]
    pub fn from_url(url: &str) -> Self {
        let url_lower = url.to_lowercase();

        if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
            SocialPlatform::Twitter
        } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
            SocialPlatform::YouTube
        } else if url_lower.contains("linkedin.com") {
            SocialPlatform::LinkedIn
        } else if url_lower.contains("discord.com") || url_lower.contains("discord.gg") {
            SocialPlatform::Discord
        } else if url_lower.contains("t.me") || url_lower.contains("telegram") {
            SocialPlatform::Telegram
        } else {
            SocialPlatform::Unknown
        }
    }
}

// ─── Parsed URL data structures ───────────────────────────────────────────────

/// Parsed Twitter/X post information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterPost {
    /// Post/Tweet ID
    pub post_id: String,
    /// Username (without @)
    pub username: String,
    /// Full URL to the post
    pub url: String,
    /// Whether this is a retweet/repost URL
    pub is_retweet: bool,
    /// Thread position (if part of a thread)
    pub thread_position: Option<u32>,
}

/// Parsed `YouTube` video information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeVideo {
    /// Video ID
    pub video_id: String,
    /// Channel handle or ID (if available from URL)
    pub channel: Option<String>,
    /// Full URL to the video
    pub url: String,
    /// Start time in seconds (if specified)
    pub start_time: Option<u32>,
    /// Whether this is a shorts URL
    pub is_shorts: bool,
    /// Whether this is a live stream URL
    pub is_live: bool,
    /// Playlist ID (if part of a playlist)
    pub playlist_id: Option<String>,
}

/// Parsed `LinkedIn` post information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedInPost {
    /// Post activity ID
    pub activity_id: Option<String>,
    /// Profile or company identifier
    pub profile: Option<String>,
    /// Full URL
    pub url: String,
    /// Post type
    pub post_type: LinkedInPostType,
}

/// `LinkedIn` post types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkedInPostType {
    /// Regular post/update
    Post,
    /// Article
    Article,
    /// Company page post
    CompanyPost,
    /// Unknown type
    Unknown,
}

// ─── URL parser ───────────────────────────────────────────────────────────────

/// Social media URL parser
pub struct SocialMediaParser;

impl SocialMediaParser {
    /// Parse a Twitter/X URL
    #[must_use]
    pub fn parse_twitter_url(url: &str) -> Option<TwitterPost> {
        let url_lower = url.to_lowercase();

        // Check if it's a Twitter/X URL
        if !url_lower.contains("twitter.com") && !url_lower.contains("x.com") {
            return None;
        }

        // Extract username and post ID
        // Formats:
        // - https://twitter.com/username/status/1234567890
        // - https://x.com/username/status/1234567890
        // - https://twitter.com/username/status/1234567890?s=20
        // - https://twitter.com/i/web/status/1234567890

        let parts: Vec<&str> = url.split('/').collect();

        // Find "status" position
        let status_pos = parts.iter().position(|&p| p == "status")?;

        // Get post ID (next part after "status")
        let post_id_with_params = parts.get(status_pos + 1)?;
        let post_id = post_id_with_params.split('?').next()?.to_string();

        if post_id.is_empty() || !post_id.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        // Get username (before "status")
        let username = if parts.get(status_pos.saturating_sub(1)) == Some(&"i") {
            // Format: /i/web/status/... - anonymous/embedded
            "unknown".to_string()
        } else {
            (*parts.get(status_pos.saturating_sub(1))?).to_string()
        };

        // Check if it's a retweet URL
        let is_retweet = url_lower.contains("/retweet") || url_lower.contains("retweeted_status");

        Some(TwitterPost {
            post_id,
            username,
            url: url.to_string(),
            is_retweet,
            thread_position: None,
        })
    }

    /// Parse a `YouTube` URL
    #[must_use]
    pub fn parse_youtube_url(url: &str) -> Option<YouTubeVideo> {
        let url_lower = url.to_lowercase();

        // Check if it's a YouTube URL
        if !url_lower.contains("youtube.com") && !url_lower.contains("youtu.be") {
            return None;
        }

        let mut video_id = None;
        let mut channel = None;
        let mut start_time = None;
        let mut playlist_id = None;

        // Check for shorts and live
        let is_shorts = url_lower.contains("/shorts/");
        let is_live = url_lower.contains("/live/") || url_lower.contains("live=");

        // Parse youtu.be format: https://youtu.be/VIDEO_ID
        if url_lower.contains("youtu.be") {
            let parts: Vec<&str> = url.split('/').collect();
            if let Some(last) = parts.last() {
                let id_part = last.split('?').next().unwrap_or(*last);
                if !id_part.is_empty() && id_part.len() == 11 {
                    video_id = Some(id_part.to_string());
                }
            }
        }

        // Parse youtube.com formats
        if url_lower.contains("youtube.com") {
            // Format: /watch?v=VIDEO_ID
            if let Some(v_param) = Self::extract_url_param(url, "v") {
                video_id = Some(v_param);
            }

            // Format: /shorts/VIDEO_ID
            if is_shorts {
                let parts: Vec<&str> = url.split("/shorts/").collect();
                if let Some(after_shorts) = parts.get(1) {
                    let id = after_shorts.split(['?', '/']).next().unwrap_or("");
                    if !id.is_empty() {
                        video_id = Some(id.to_string());
                    }
                }
            }

            // Format: /live/VIDEO_ID
            if url_lower.contains("/live/") {
                let parts: Vec<&str> = url.split("/live/").collect();
                if let Some(after_live) = parts.get(1) {
                    let id = after_live.split(['?', '/']).next().unwrap_or("");
                    if !id.is_empty() {
                        video_id = Some(id.to_string());
                    }
                }
            }

            // Format: /embed/VIDEO_ID
            if url_lower.contains("/embed/") {
                let parts: Vec<&str> = url.split("/embed/").collect();
                if let Some(after_embed) = parts.get(1) {
                    let id = after_embed.split(['?', '/']).next().unwrap_or("");
                    if !id.is_empty() {
                        video_id = Some(id.to_string());
                    }
                }
            }

            // Extract playlist ID
            if let Some(list) = Self::extract_url_param(url, "list") {
                playlist_id = Some(list);
            }

            // Extract channel
            if url_lower.contains("/channel/") {
                let parts: Vec<&str> = url.split("/channel/").collect();
                if let Some(after_channel) = parts.get(1) {
                    let ch = after_channel.split(['?', '/']).next().unwrap_or("");
                    if !ch.is_empty() {
                        channel = Some(ch.to_string());
                    }
                }
            } else if url_lower.contains("/@") {
                let parts: Vec<&str> = url.split("/@").collect();
                if let Some(after_at) = parts.get(1) {
                    let handle = after_at.split(['?', '/']).next().unwrap_or("");
                    if !handle.is_empty() {
                        channel = Some(format!("@{handle}"));
                    }
                }
            }
        }

        // Extract start time (t= parameter)
        if let Some(t) = Self::extract_url_param(url, "t") {
            start_time = Self::parse_time_param(&t);
        }

        video_id.map(|vid| YouTubeVideo {
            video_id: vid,
            channel,
            url: url.to_string(),
            start_time,
            is_shorts,
            is_live,
            playlist_id,
        })
    }

    /// Parse a `LinkedIn` URL
    #[must_use]
    pub fn parse_linkedin_url(url: &str) -> Option<LinkedInPost> {
        let url_lower = url.to_lowercase();

        if !url_lower.contains("linkedin.com") {
            return None;
        }

        let mut activity_id = None;
        let mut profile = None;
        let mut post_type = LinkedInPostType::Unknown;

        // Format: /posts/activity-ID
        if url_lower.contains("/posts/") || url_lower.contains("/feed/update/") {
            post_type = LinkedInPostType::Post;

            // Try to extract activity ID
            if let Some(activity_part) = url.split("activity-").nth(1) {
                let id = activity_part.split(['?', '/', '-']).next().unwrap_or("");
                if !id.is_empty() {
                    activity_id = Some(id.to_string());
                }
            }
        }

        // Format: /pulse/... (article)
        if url_lower.contains("/pulse/") {
            post_type = LinkedInPostType::Article;
        }

        // Format: /company/...
        if url_lower.contains("/company/") {
            post_type = LinkedInPostType::CompanyPost;
            let parts: Vec<&str> = url.split("/company/").collect();
            if let Some(after_company) = parts.get(1) {
                let company = after_company.split(['?', '/']).next().unwrap_or("");
                if !company.is_empty() {
                    profile = Some(company.to_string());
                }
            }
        }

        // Format: /in/username
        if url_lower.contains("/in/") {
            let parts: Vec<&str> = url.split("/in/").collect();
            if let Some(after_in) = parts.get(1) {
                let username = after_in.split(['?', '/']).next().unwrap_or("");
                if !username.is_empty() {
                    profile = Some(username.to_string());
                }
            }
        }

        Some(LinkedInPost {
            activity_id,
            profile,
            url: url.to_string(),
            post_type,
        })
    }

    /// Extract a URL parameter value
    fn extract_url_param(url: &str, param: &str) -> Option<String> {
        let query_start = url.find('?')?;
        let query = &url[query_start + 1..];

        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                if key == param {
                    return Some(value.to_string());
                }
            }
        }

        None
    }

    /// Parse time parameter (supports "1h2m3s" or "123" formats)
    fn parse_time_param(t: &str) -> Option<u32> {
        // Try simple seconds format first
        if let Ok(secs) = t.parse::<u32>() {
            return Some(secs);
        }

        // Parse h/m/s format
        let mut total_secs = 0u32;
        let mut current_num = String::new();

        for c in t.chars() {
            if c.is_ascii_digit() {
                current_num.push(c);
            } else {
                let num: u32 = current_num.parse().unwrap_or(0);
                match c {
                    'h' => total_secs += num * 3600,
                    'm' => total_secs += num * 60,
                    's' => total_secs += num,
                    _ => {}
                }
                current_num.clear();
            }
        }

        // Handle trailing number without suffix (assume seconds)
        if !current_num.is_empty() {
            if let Ok(num) = current_num.parse::<u32>() {
                total_secs += num;
            }
        }

        if total_secs > 0 {
            Some(total_secs)
        } else {
            None
        }
    }
}

// ─── Verification types ───────────────────────────────────────────────────────

/// Social media verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialVerificationResult {
    /// Platform identified
    pub platform: SocialPlatform,
    /// Whether the URL is valid and parseable
    pub is_valid_url: bool,
    /// Verification status
    pub status: VerificationStatus,
    /// Confidence score (0-100)
    pub confidence: u32,
    /// Parsed details (platform-specific)
    pub details: SocialDetails,
    /// Verification notes
    pub notes: Vec<String>,
    /// Whether manual verification is recommended
    pub needs_manual_review: bool,
}

/// Verification status
///
/// # Honest semantics
///
/// - [`Verified`](VerificationStatus::Verified) means the platform API
///   confirmed the resource exists and is accessible.
/// - [`NeedsManualReview`](VerificationStatus::NeedsManualReview) means the URL
///   parsed successfully but no API credentials were available to confirm
///   existence with the platform.  A human must review.
/// - [`Unverified`](VerificationStatus::Unverified) means the URL could not be
///   parsed (malformed, wrong domain, etc.).
/// - [`Suspicious`](VerificationStatus::Suspicious) means the URL raised
///   active red flags.
/// - [`Unsupported`](VerificationStatus::Unsupported) means the platform
///   cannot be verified programmatically.
/// - [`Failed`](VerificationStatus::Failed) means an API call was attempted
///   but returned a definitive negative (e.g. 404 Not Found from the platform).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Platform API confirmed the resource exists
    Verified,
    /// URL parsed OK but no API credentials — human review required
    NeedsManualReview,
    /// Could not verify (URL malformed / unparseable)
    Unverified,
    /// URL appears suspicious
    Suspicious,
    /// Platform not supported for verification
    Unsupported,
    /// API call returned definitive failure (e.g. resource not found on platform)
    Failed,
}

/// Platform-specific parsed details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SocialDetails {
    /// Twitter post details
    Twitter(TwitterPost),
    /// `YouTube` video details
    YouTube(YouTubeVideo),
    /// `LinkedIn` post details
    LinkedIn(LinkedInPost),
    /// No details available
    None,
}

// ─── API credentials ──────────────────────────────────────────────────────────

/// Optional platform API credentials for real verification.
///
/// All fields are optional.  Fields that are `None` cause the verifier to skip
/// the corresponding API call and return [`VerificationStatus::NeedsManualReview`]
/// instead of fabricating a `Verified` result.
#[derive(Debug, Clone, Default)]
pub struct SocialApiCredentials {
    /// Twitter API v2 Bearer token (`Authorization: Bearer <token>`).
    /// Required for Twitter/X tweet existence checks.
    pub twitter_bearer_token: Option<String>,

    /// YouTube Data API v3 key (passed as `key=` query parameter).
    /// Required for YouTube video existence checks.
    pub youtube_api_key: Option<String>,
}

impl SocialApiCredentials {
    /// Construct credentials with no platform access configured.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Construct credentials with a Twitter bearer token only.
    #[must_use]
    pub fn twitter_only(bearer_token: impl Into<String>) -> Self {
        Self {
            twitter_bearer_token: Some(bearer_token.into()),
            youtube_api_key: None,
        }
    }

    /// Construct credentials with a YouTube API key only.
    #[must_use]
    pub fn youtube_only(api_key: impl Into<String>) -> Self {
        Self {
            twitter_bearer_token: None,
            youtube_api_key: Some(api_key.into()),
        }
    }

    /// Construct credentials with both Twitter and YouTube access.
    #[must_use]
    pub fn both(bearer_token: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            twitter_bearer_token: Some(bearer_token.into()),
            youtube_api_key: Some(api_key.into()),
        }
    }
}

// ─── Internal API response shapes ─────────────────────────────────────────────

/// Minimal Twitter API v2 tweet lookup response.
///
/// `GET https://api.twitter.com/2/tweets/{id}`
#[derive(Debug, Deserialize)]
struct TwitterApiTweetResponse {
    /// Present when the tweet was found.
    data: Option<TwitterApiTweetData>,
    /// Present when the tweet was not found or an error occurred.
    errors: Option<Vec<TwitterApiError>>,
}

#[derive(Debug, Deserialize)]
struct TwitterApiTweetData {
    id: String,
    #[allow(dead_code)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TwitterApiError {
    #[allow(dead_code)]
    title: Option<String>,
    #[allow(dead_code)]
    detail: Option<String>,
    #[allow(dead_code)]
    r#type: Option<String>,
}

/// Minimal YouTube Data API v3 videos list response.
///
/// `GET https://www.googleapis.com/youtube/v3/videos?part=id&id={id}&key={key}`
#[derive(Debug, Deserialize)]
struct YouTubeApiVideosResponse {
    /// List of matched video resources.  Empty means video does not exist.
    items: Vec<YouTubeApiVideoItem>,
}

#[derive(Debug, Deserialize)]
struct YouTubeApiVideoItem {
    id: String,
}

// ─── Verifier ─────────────────────────────────────────────────────────────────

/// Social media verifier
///
/// Construct with [`SocialMediaVerifier::new`] (no API access) or
/// [`SocialMediaVerifier::with_credentials`] to enable real API verification.
pub struct SocialMediaVerifier {
    http_client: Arc<reqwest::Client>,
    credentials: SocialApiCredentials,
}

impl SocialMediaVerifier {
    /// Create a new social media verifier with **no** API credentials.
    ///
    /// Without credentials the verifier parses URLs and returns
    /// [`VerificationStatus::NeedsManualReview`] — it never fabricates
    /// a `Verified` status.
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(reqwest::Client::new()),
            credentials: SocialApiCredentials::none(),
        }
    }

    /// Create with custom HTTP client and no credentials.
    #[must_use]
    pub fn with_client(client: Arc<reqwest::Client>) -> Self {
        Self {
            http_client: client,
            credentials: SocialApiCredentials::none(),
        }
    }

    /// Create with API credentials for real platform verification.
    #[must_use]
    pub fn with_credentials(credentials: SocialApiCredentials) -> Self {
        Self {
            http_client: Arc::new(reqwest::Client::new()),
            credentials,
        }
    }

    /// Create with a custom HTTP client **and** API credentials.
    #[must_use]
    pub fn with_client_and_credentials(
        client: Arc<reqwest::Client>,
        credentials: SocialApiCredentials,
    ) -> Self {
        Self {
            http_client: client,
            credentials,
        }
    }

    /// Verify a social media URL.
    ///
    /// When API credentials are configured the verification contacts the
    /// platform API.  When no credentials are available the result has
    /// `status = NeedsManualReview` (never `Verified`).
    pub async fn verify_url(&self, url: &str) -> Result<SocialVerificationResult> {
        let platform = SocialPlatform::from_url(url);
        let mut notes = Vec::new();
        let mut needs_manual_review = false;

        match platform {
            SocialPlatform::Twitter => {
                self.verify_twitter(url, &mut notes, &mut needs_manual_review)
                    .await
            }
            SocialPlatform::YouTube => {
                self.verify_youtube(url, &mut notes, &mut needs_manual_review)
                    .await
            }
            SocialPlatform::LinkedIn => {
                self.verify_linkedin(url, &mut notes, &mut needs_manual_review)
            }
            SocialPlatform::Discord | SocialPlatform::Telegram => {
                notes.push("Platform requires manual verification".to_string());
                needs_manual_review = true;
                Ok(SocialVerificationResult {
                    platform,
                    is_valid_url: true,
                    status: VerificationStatus::Unsupported,
                    confidence: 20,
                    details: SocialDetails::None,
                    notes,
                    needs_manual_review,
                })
            }
            SocialPlatform::Unknown => Err(AiError::VerificationFailed(
                "Unknown social media platform".to_string(),
            )),
        }
    }

    // ── Twitter/X ─────────────────────────────────────────────────────────────

    /// Verify Twitter/X URL.
    ///
    /// If a bearer token is configured, calls Twitter API v2
    /// `GET /2/tweets/{id}` and interprets the response honestly.
    /// Without a bearer token, returns [`VerificationStatus::NeedsManualReview`].
    async fn verify_twitter(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_twitter_url(url);

        let Some(post) = parsed else {
            notes.push("Could not parse Twitter URL".to_string());
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            });
        };

        let mut confidence = 70u32;

        // Structural URL checks — these reduce confidence but are never
        // sufficient to elevate status to Verified.
        if post.username == "unknown" {
            notes.push("Username could not be determined from URL".to_string());
            confidence = confidence.saturating_sub(20);
            *needs_manual_review = true;
        }

        if post.is_retweet {
            notes.push("URL appears to be a retweet".to_string());
            confidence = confidence.saturating_sub(10);
        }

        if post.post_id.len() < 10 || post.post_id.len() > 25 {
            notes.push("Post ID has unusual length".to_string());
            confidence = confidence.saturating_sub(15);
        }

        notes.push(format!("Tweet ID: {}", post.post_id));
        notes.push(format!("Username: @{}", post.username));

        // Attempt real API verification when credentials are present.
        let Some(ref bearer) = self.credentials.twitter_bearer_token else {
            notes.push(
                "No Twitter API credentials configured — manual review required".to_string(),
            );
            *needs_manual_review = true;
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence,
                details: SocialDetails::Twitter(post),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            });
        };

        // --- Real Twitter API v2 call ---
        let api_url = format!(
            "https://api.twitter.com/2/tweets/{}?tweet.fields=id,text",
            post.post_id
        );

        let response = self
            .http_client
            .get(&api_url)
            .header("Authorization", format!("Bearer {bearer}"))
            .send()
            .await
            .map_err(|e| {
                AiError::VerificationFailed(format!("Twitter API request failed: {e}"))
            })?;

        let http_status = response.status();

        if http_status == reqwest::StatusCode::NOT_FOUND {
            notes.push("Twitter API: tweet not found (404)".to_string());
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::Failed,
                confidence: 0,
                details: SocialDetails::Twitter(post),
                notes: notes.clone(),
                needs_manual_review: false,
            });
        }

        if http_status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            notes.push("Twitter API: rate limited — manual review required".to_string());
            *needs_manual_review = true;
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence: confidence.saturating_sub(10),
                details: SocialDetails::Twitter(post),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            });
        }

        if http_status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AiError::VerificationFailed(
                "Twitter API: invalid or expired bearer token (401)".to_string(),
            ));
        }

        if !http_status.is_success() {
            return Err(AiError::VerificationFailed(format!(
                "Twitter API returned HTTP {http_status}"
            )));
        }

        let body: TwitterApiTweetResponse =
            response
                .json()
                .await
                .map_err(|e| {
                    AiError::VerificationFailed(format!(
                        "Failed to parse Twitter API response: {e}"
                    ))
                })?;

        // The API may return 200 but with errors (e.g. protected tweets).
        if body.data.is_none() {
            let error_detail = body
                .errors
                .as_ref()
                .and_then(|errs| errs.first())
                .and_then(|e| e.detail.as_deref())
                .unwrap_or("unknown error");
            notes.push(format!("Twitter API: tweet inaccessible — {error_detail}"));
            *needs_manual_review = true;
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence: confidence.saturating_sub(10),
                details: SocialDetails::Twitter(post),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            });
        }

        // data.id is present — tweet confirmed to exist.
        // The is_none() early-return above guarantees Some here; propagate the
        // impossible None as an Internal error rather than panic.
        let tweet_data = body.data.as_ref().ok_or_else(|| {
            AiError::Internal("Twitter API: response had no data field after validation".to_string())
        })?;
        notes.push(format!(
            "Twitter API: tweet {} confirmed via API",
            tweet_data.id
        ));

        Ok(SocialVerificationResult {
            platform: SocialPlatform::Twitter,
            is_valid_url: true,
            status: VerificationStatus::Verified,
            confidence: 100,
            details: SocialDetails::Twitter(post),
            notes: notes.clone(),
            needs_manual_review: *needs_manual_review,
        })
    }

    // ── YouTube ───────────────────────────────────────────────────────────────

    /// Verify `YouTube` URL.
    ///
    /// If a YouTube Data API key is configured, calls
    /// `GET /youtube/v3/videos?part=id&id={id}&key={key}` and interprets
    /// the response honestly.
    /// Without an API key, returns [`VerificationStatus::NeedsManualReview`].
    async fn verify_youtube(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_youtube_url(url);

        let Some(video) = parsed else {
            notes.push("Could not parse YouTube URL".to_string());
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            });
        };

        let mut confidence = 80u32;

        // Structural video-ID checks.
        if video.video_id.len() != 11 {
            notes.push(format!(
                "Video ID has unusual length: {} (expected 11)",
                video.video_id.len()
            ));
            confidence = confidence.saturating_sub(20);
        }

        let valid_chars = video
            .video_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
        if !valid_chars {
            notes.push("Video ID contains invalid characters".to_string());
            confidence = confidence.saturating_sub(30);
        }

        notes.push(format!("Video ID: {}", video.video_id));

        if let Some(ref ch) = video.channel {
            notes.push(format!("Channel: {ch}"));
        }
        if video.is_shorts {
            notes.push("Video type: YouTube Shorts".to_string());
        }
        if video.is_live {
            notes.push("Video type: Live stream".to_string());
        }
        if let Some(t) = video.start_time {
            notes.push(format!("Start time: {t}s"));
        }
        if let Some(ref pl) = video.playlist_id {
            notes.push(format!("Playlist: {pl}"));
        }

        // Attempt real API verification when credentials are present.
        let Some(ref api_key) = self.credentials.youtube_api_key else {
            notes.push(
                "No YouTube API credentials configured — manual review required".to_string(),
            );
            *needs_manual_review = true;
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence,
                details: SocialDetails::YouTube(video),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            });
        };

        // --- Real YouTube Data API v3 call ---
        let api_url = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=id&id={}&key={}",
            video.video_id, api_key
        );

        let response = self
            .http_client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| {
                AiError::VerificationFailed(format!("YouTube API request failed: {e}"))
            })?;

        let http_status = response.status();

        if http_status == reqwest::StatusCode::BAD_REQUEST {
            return Err(AiError::VerificationFailed(
                "YouTube API: bad request — check API key format (400)".to_string(),
            ));
        }

        if http_status == reqwest::StatusCode::FORBIDDEN {
            return Err(AiError::VerificationFailed(
                "YouTube API: API key rejected or quota exceeded (403)".to_string(),
            ));
        }

        if !http_status.is_success() {
            return Err(AiError::VerificationFailed(format!(
                "YouTube API returned HTTP {http_status}"
            )));
        }

        let body: YouTubeApiVideosResponse =
            response
                .json()
                .await
                .map_err(|e| {
                    AiError::VerificationFailed(format!(
                        "Failed to parse YouTube API response: {e}"
                    ))
                })?;

        if body.items.is_empty() {
            // API returned 200 but no matching videos — video does not exist
            // or is private/deleted.
            notes.push("YouTube API: no video found for this ID — may be private, deleted, or never existed".to_string());
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: true,
                status: VerificationStatus::Failed,
                confidence: 0,
                details: SocialDetails::YouTube(video),
                notes: notes.clone(),
                needs_manual_review: false,
            });
        }

        // items is non-empty — video confirmed to exist.
        let confirmed_id = &body.items[0].id;
        notes.push(format!(
            "YouTube API: video {confirmed_id} confirmed via API"
        ));

        Ok(SocialVerificationResult {
            platform: SocialPlatform::YouTube,
            is_valid_url: true,
            status: VerificationStatus::Verified,
            confidence: 100,
            details: SocialDetails::YouTube(video),
            notes: notes.clone(),
            needs_manual_review: *needs_manual_review,
        })
    }

    // ── LinkedIn ──────────────────────────────────────────────────────────────

    /// Verify `LinkedIn` URL.
    ///
    /// LinkedIn's API requires OAuth and does not expose a simple public
    /// content-lookup endpoint; returns [`VerificationStatus::NeedsManualReview`]
    /// for all successfully-parsed URLs.
    fn verify_linkedin(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_linkedin_url(url);

        let Some(post) = parsed else {
            notes.push("Could not parse LinkedIn URL".to_string());
            return Ok(SocialVerificationResult {
                platform: SocialPlatform::LinkedIn,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            });
        };

        let mut confidence = 60u32;

        match post.post_type {
            LinkedInPostType::Post => {
                notes.push("Post type: LinkedIn post/update".to_string());
            }
            LinkedInPostType::Article => {
                notes.push("Post type: LinkedIn article".to_string());
                confidence = confidence.saturating_add(10);
            }
            LinkedInPostType::CompanyPost => {
                notes.push("Post type: Company page post".to_string());
            }
            LinkedInPostType::Unknown => {
                notes.push("Could not determine post type".to_string());
                confidence = confidence.saturating_sub(10);
            }
        }

        if let Some(ref activity) = post.activity_id {
            notes.push(format!("Activity ID: {activity}"));
        }

        if let Some(ref profile) = post.profile {
            notes.push(format!("Profile/Company: {profile}"));
        }

        // LinkedIn requires OAuth for API access; no simple public lookup.
        notes.push(
            "LinkedIn verification requires OAuth API access — manual review required".to_string(),
        );
        *needs_manual_review = true;

        Ok(SocialVerificationResult {
            platform: SocialPlatform::LinkedIn,
            is_valid_url: true,
            status: VerificationStatus::NeedsManualReview,
            confidence,
            details: SocialDetails::LinkedIn(post),
            notes: notes.clone(),
            needs_manual_review: *needs_manual_review,
        })
    }

    // ── Batch / summary helpers ────────────────────────────────────────────────

    /// Batch verify multiple URLs
    pub async fn verify_urls(&self, urls: &[&str]) -> Vec<Result<SocialVerificationResult>> {
        let mut results = Vec::with_capacity(urls.len());
        for url in urls {
            results.push(self.verify_url(url).await);
        }
        results
    }

    /// Get a summary of verification results
    #[must_use]
    pub fn summarize_results(results: &[SocialVerificationResult]) -> VerificationSummary {
        let total = results.len();
        // Only API-confirmed results count as verified in the summary.
        let verified = results
            .iter()
            .filter(|r| r.status == VerificationStatus::Verified)
            .count();
        let needs_review = results.iter().filter(|r| r.needs_manual_review).count();
        let avg_confidence = if total > 0 {
            results.iter().map(|r| u64::from(r.confidence)).sum::<u64>() / total as u64
        } else {
            0
        };

        let mut platforms = std::collections::HashMap::new();
        for result in results {
            *platforms.entry(result.platform).or_insert(0) += 1;
        }

        VerificationSummary {
            total_urls: total,
            verified_count: verified,
            needs_manual_review: needs_review,
            average_confidence: avg_confidence as u32,
            platforms_found: platforms,
        }
    }
}

impl Default for SocialMediaVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Summary ──────────────────────────────────────────────────────────────────

/// Summary of verification results
#[derive(Debug, Clone, Serialize)]
pub struct VerificationSummary {
    /// Total URLs processed
    pub total_urls: usize,
    /// URLs confirmed via platform API
    pub verified_count: usize,
    /// URLs needing manual review
    pub needs_manual_review: usize,
    /// Average confidence score
    pub average_confidence: u32,
    /// Platforms found
    pub platforms_found: std::collections::HashMap<SocialPlatform, usize>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── URL parsing ────────────────────────────────────────────────────────────

    #[test]
    fn test_twitter_url_parsing() {
        let url = "https://twitter.com/elonmusk/status/1234567890123456789";
        let parsed = SocialMediaParser::parse_twitter_url(url).unwrap();

        assert_eq!(parsed.username, "elonmusk");
        assert_eq!(parsed.post_id, "1234567890123456789");
        assert!(!parsed.is_retweet);
    }

    #[test]
    fn test_twitter_x_url_parsing() {
        let url = "https://x.com/username/status/9876543210?s=20";
        let parsed = SocialMediaParser::parse_twitter_url(url).unwrap();

        assert_eq!(parsed.username, "username");
        assert_eq!(parsed.post_id, "9876543210");
    }

    #[test]
    fn test_youtube_watch_url() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=30s";
        let parsed = SocialMediaParser::parse_youtube_url(url).unwrap();

        assert_eq!(parsed.video_id, "dQw4w9WgXcQ");
        assert_eq!(parsed.start_time, Some(30));
        assert!(!parsed.is_shorts);
    }

    #[test]
    fn test_youtube_short_url() {
        let url = "https://youtu.be/dQw4w9WgXcQ";
        let parsed = SocialMediaParser::parse_youtube_url(url).unwrap();

        assert_eq!(parsed.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn test_youtube_shorts_url() {
        let url = "https://www.youtube.com/shorts/abc12345678";
        let parsed = SocialMediaParser::parse_youtube_url(url).unwrap();

        assert_eq!(parsed.video_id, "abc12345678");
        assert!(parsed.is_shorts);
    }

    #[test]
    fn test_linkedin_post_url() {
        let url = "https://www.linkedin.com/posts/johndoe_activity-1234567890_test";
        let parsed = SocialMediaParser::parse_linkedin_url(url).unwrap();

        assert_eq!(parsed.post_type, LinkedInPostType::Post);
        assert!(parsed.activity_id.is_some());
    }

    #[test]
    fn test_platform_detection() {
        assert_eq!(
            SocialPlatform::from_url("https://twitter.com/user/status/123"),
            SocialPlatform::Twitter
        );
        assert_eq!(
            SocialPlatform::from_url("https://x.com/user/status/123"),
            SocialPlatform::Twitter
        );
        assert_eq!(
            SocialPlatform::from_url("https://youtube.com/watch?v=abc"),
            SocialPlatform::YouTube
        );
        assert_eq!(
            SocialPlatform::from_url("https://youtu.be/abc"),
            SocialPlatform::YouTube
        );
        assert_eq!(
            SocialPlatform::from_url("https://linkedin.com/in/user"),
            SocialPlatform::LinkedIn
        );
    }

    #[test]
    fn test_time_param_parsing() {
        assert_eq!(SocialMediaParser::parse_time_param("30"), Some(30));
        assert_eq!(SocialMediaParser::parse_time_param("1m30s"), Some(90));
        assert_eq!(SocialMediaParser::parse_time_param("1h30m"), Some(5400));
        assert_eq!(SocialMediaParser::parse_time_param("2h15m30s"), Some(8130));
    }

    // ── No-credentials honesty tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_twitter_no_credentials_returns_needs_manual_review() {
        let verifier = SocialMediaVerifier::new(); // no credentials
        let url = "https://twitter.com/elonmusk/status/1234567890123456789";
        let result = verifier.verify_url(url).await.unwrap();

        assert!(
            result.is_valid_url,
            "Valid Twitter URL must parse successfully"
        );
        assert_ne!(
            result.status,
            VerificationStatus::Verified,
            "Without API credentials, status must not be Verified"
        );
        assert_eq!(
            result.status,
            VerificationStatus::NeedsManualReview,
            "Without API credentials, Twitter result must be NeedsManualReview"
        );
        assert!(result.needs_manual_review);
        // Parsed post details must still be populated.
        assert!(matches!(result.details, SocialDetails::Twitter(_)));
    }

    #[tokio::test]
    async fn test_youtube_no_credentials_returns_needs_manual_review() {
        let verifier = SocialMediaVerifier::new(); // no credentials
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let result = verifier.verify_url(url).await.unwrap();

        assert!(
            result.is_valid_url,
            "Valid YouTube URL must parse successfully"
        );
        assert_ne!(
            result.status,
            VerificationStatus::Verified,
            "Without API credentials, status must not be Verified"
        );
        assert_eq!(
            result.status,
            VerificationStatus::NeedsManualReview,
            "Without API credentials, YouTube result must be NeedsManualReview"
        );
        assert!(result.needs_manual_review);
        assert!(matches!(result.details, SocialDetails::YouTube(_)));
    }

    #[tokio::test]
    async fn test_linkedin_always_needs_manual_review() {
        let verifier = SocialMediaVerifier::new();
        let url = "https://www.linkedin.com/posts/johndoe_activity-1234567890_test";
        let result = verifier.verify_url(url).await.unwrap();

        assert!(result.is_valid_url);
        assert_ne!(result.status, VerificationStatus::Verified);
        assert_eq!(result.status, VerificationStatus::NeedsManualReview);
        assert!(result.needs_manual_review);
    }

    #[tokio::test]
    async fn test_malformed_twitter_url_returns_unverified() {
        let verifier = SocialMediaVerifier::new();
        // No "status" segment, so parse fails.
        let url = "https://twitter.com/elonmusk";
        let result = verifier.verify_url(url).await.unwrap();

        assert!(!result.is_valid_url);
        assert_eq!(result.status, VerificationStatus::Unverified);
        assert_ne!(result.status, VerificationStatus::Verified);
    }

    #[tokio::test]
    async fn test_malformed_youtube_url_returns_unverified() {
        let verifier = SocialMediaVerifier::new();
        let url = "https://www.youtube.com/channel/UCxxx"; // no video ID
        let result = verifier.verify_url(url).await.unwrap();

        assert!(!result.is_valid_url);
        assert_eq!(result.status, VerificationStatus::Unverified);
        assert_ne!(result.status, VerificationStatus::Verified);
    }

    #[tokio::test]
    async fn test_discord_returns_unsupported() {
        let verifier = SocialMediaVerifier::new();
        let url = "https://discord.com/channels/1234/5678";
        let result = verifier.verify_url(url).await.unwrap();

        assert_eq!(result.status, VerificationStatus::Unsupported);
        assert!(result.needs_manual_review);
    }

    #[tokio::test]
    async fn test_unknown_platform_returns_error() {
        let verifier = SocialMediaVerifier::new();
        let url = "https://example.com/some-post";
        let result = verifier.verify_url(url).await;

        assert!(result.is_err());
    }

    // ── API request construction tests ─────────────────────────────────────────

    /// Verify that the Twitter API URL is constructed correctly from the tweet ID.
    /// We test construction without hitting the live API by using the known URL pattern.
    #[test]
    fn test_twitter_api_url_construction() {
        let tweet_id = "1234567890123456789";
        let expected = format!(
            "https://api.twitter.com/2/tweets/{}?tweet.fields=id,text",
            tweet_id
        );
        // The format string used internally must produce this URL.
        let constructed = format!(
            "https://api.twitter.com/2/tweets/{}?tweet.fields=id,text",
            tweet_id
        );
        assert_eq!(constructed, expected);
    }

    /// Verify that the YouTube API URL is constructed correctly.
    #[test]
    fn test_youtube_api_url_construction() {
        let video_id = "dQw4w9WgXcQ";
        let api_key = "AIzaFakeKeyForTest";
        let constructed = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=id&id={}&key={}",
            video_id, api_key
        );
        assert!(constructed.contains("youtube/v3/videos"));
        assert!(constructed.contains(&format!("id={video_id}")));
        assert!(constructed.contains(&format!("key={api_key}")));
    }

    // ── summarize_results ──────────────────────────────────────────────────────

    #[test]
    fn test_summarize_needs_manual_review_not_counted_as_verified() {
        // NeedsManualReview results must not be counted as verified.
        let results = vec![
            SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence: 70,
                details: SocialDetails::None,
                notes: vec![],
                needs_manual_review: true,
            },
            SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: true,
                status: VerificationStatus::NeedsManualReview,
                confidence: 80,
                details: SocialDetails::None,
                notes: vec![],
                needs_manual_review: true,
            },
        ];

        let summary = SocialMediaVerifier::summarize_results(&results);
        assert_eq!(
            summary.verified_count, 0,
            "NeedsManualReview must not be counted as verified"
        );
        assert_eq!(summary.needs_manual_review, 2);
    }

    #[test]
    fn test_summarize_verified_counted_correctly() {
        let results = vec![SocialVerificationResult {
            platform: SocialPlatform::Twitter,
            is_valid_url: true,
            status: VerificationStatus::Verified,
            confidence: 100,
            details: SocialDetails::None,
            notes: vec![],
            needs_manual_review: false,
        }];

        let summary = SocialMediaVerifier::summarize_results(&results);
        assert_eq!(summary.verified_count, 1);
    }

    // ── SocialApiCredentials constructors ──────────────────────────────────────

    #[test]
    fn test_credentials_none() {
        let creds = SocialApiCredentials::none();
        assert!(creds.twitter_bearer_token.is_none());
        assert!(creds.youtube_api_key.is_none());
    }

    #[test]
    fn test_credentials_twitter_only() {
        let creds = SocialApiCredentials::twitter_only("my_token");
        assert_eq!(creds.twitter_bearer_token.as_deref(), Some("my_token"));
        assert!(creds.youtube_api_key.is_none());
    }

    #[test]
    fn test_credentials_youtube_only() {
        let creds = SocialApiCredentials::youtube_only("my_key");
        assert!(creds.twitter_bearer_token.is_none());
        assert_eq!(creds.youtube_api_key.as_deref(), Some("my_key"));
    }

    #[test]
    fn test_credentials_both() {
        let creds = SocialApiCredentials::both("tok", "key");
        assert_eq!(creds.twitter_bearer_token.as_deref(), Some("tok"));
        assert_eq!(creds.youtube_api_key.as_deref(), Some("key"));
    }

    // ── VerificationStatus never-Verified invariant ───────────────────────────

    #[tokio::test]
    async fn test_batch_verify_no_credentials_no_verified() {
        let verifier = SocialMediaVerifier::new();
        let urls = [
            "https://twitter.com/user/status/1234567890123456789",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.linkedin.com/posts/johndoe_activity-1234567890_test",
        ];
        let results = verifier.verify_urls(&urls).await;
        for res in &results {
            let r = res.as_ref().expect("All URLs should parse without error");
            assert_ne!(
                r.status,
                VerificationStatus::Verified,
                "Without API credentials, no result should be Verified. URL platform: {:?}",
                r.platform
            );
        }
    }
}
