//! Social media verification module
//!
//! This module provides verification capabilities for social media evidence
//! including Twitter/X posts and `YouTube` videos.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AiError, Result};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// URL parsed and appears valid
    Verified,
    /// Could not verify, needs manual check
    Unverified,
    /// URL appears suspicious
    Suspicious,
    /// Platform not supported for verification
    Unsupported,
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

/// Social media verifier
pub struct SocialMediaVerifier {
    #[allow(dead_code)]
    http_client: Arc<reqwest::Client>,
}

impl SocialMediaVerifier {
    /// Create a new social media verifier
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(reqwest::Client::new()),
        }
    }

    /// Create with custom HTTP client
    #[must_use]
    pub fn with_client(client: Arc<reqwest::Client>) -> Self {
        Self {
            http_client: client,
        }
    }

    /// Verify a social media URL
    pub fn verify_url(&self, url: &str) -> Result<SocialVerificationResult> {
        let platform = SocialPlatform::from_url(url);
        let mut notes = Vec::new();
        let mut needs_manual_review = false;

        match platform {
            SocialPlatform::Twitter => {
                self.verify_twitter(url, &mut notes, &mut needs_manual_review)
            }
            SocialPlatform::YouTube => {
                self.verify_youtube(url, &mut notes, &mut needs_manual_review)
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

    /// Verify Twitter/X URL
    fn verify_twitter(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_twitter_url(url);

        if let Some(post) = parsed {
            let mut confidence = 70;

            // Check for suspicious patterns
            if post.username == "unknown" {
                notes.push("Username could not be determined from URL".to_string());
                confidence -= 20;
                *needs_manual_review = true;
            }

            if post.is_retweet {
                notes.push("URL appears to be a retweet".to_string());
                confidence -= 10;
            }

            // Validate post ID format (should be numeric, reasonable length)
            if post.post_id.len() < 10 || post.post_id.len() > 25 {
                notes.push("Post ID has unusual length".to_string());
                confidence -= 15;
            }

            notes.push(format!("Tweet ID: {}", post.post_id));
            notes.push(format!("Username: @{}", post.username));

            // Note: Full verification would require Twitter API access
            notes.push("Note: Content verification requires Twitter API access".to_string());
            *needs_manual_review = true;

            Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: true,
                status: VerificationStatus::Verified,
                confidence,
                details: SocialDetails::Twitter(post),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            })
        } else {
            notes.push("Could not parse Twitter URL".to_string());
            Ok(SocialVerificationResult {
                platform: SocialPlatform::Twitter,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            })
        }
    }

    /// Verify `YouTube` URL
    fn verify_youtube(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_youtube_url(url);

        if let Some(video) = parsed {
            let mut confidence = 80;

            // Validate video ID format (11 characters, alphanumeric with - and _)
            if video.video_id.len() != 11 {
                notes.push(format!(
                    "Video ID has unusual length: {} (expected 11)",
                    video.video_id.len()
                ));
                confidence -= 20;
            }

            let valid_chars = video
                .video_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
            if !valid_chars {
                notes.push("Video ID contains invalid characters".to_string());
                confidence -= 30;
            }

            notes.push(format!("Video ID: {}", video.video_id));

            if let Some(ref channel) = video.channel {
                notes.push(format!("Channel: {channel}"));
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

            if let Some(ref playlist) = video.playlist_id {
                notes.push(format!("Playlist: {playlist}"));
            }

            // Note: Full verification would require YouTube Data API
            notes.push("Note: Content verification requires YouTube API access".to_string());

            Ok(SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: true,
                status: VerificationStatus::Verified,
                confidence,
                details: SocialDetails::YouTube(video),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            })
        } else {
            notes.push("Could not parse YouTube URL".to_string());
            Ok(SocialVerificationResult {
                platform: SocialPlatform::YouTube,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            })
        }
    }

    /// Verify `LinkedIn` URL
    fn verify_linkedin(
        &self,
        url: &str,
        notes: &mut Vec<String>,
        needs_manual_review: &mut bool,
    ) -> Result<SocialVerificationResult> {
        let parsed = SocialMediaParser::parse_linkedin_url(url);

        if let Some(post) = parsed {
            let mut confidence = 60;

            match post.post_type {
                LinkedInPostType::Post => {
                    notes.push("Post type: LinkedIn post/update".to_string());
                }
                LinkedInPostType::Article => {
                    notes.push("Post type: LinkedIn article".to_string());
                    confidence += 10; // Articles are more verifiable
                }
                LinkedInPostType::CompanyPost => {
                    notes.push("Post type: Company page post".to_string());
                }
                LinkedInPostType::Unknown => {
                    notes.push("Could not determine post type".to_string());
                    confidence -= 10;
                }
            }

            if let Some(ref activity) = post.activity_id {
                notes.push(format!("Activity ID: {activity}"));
            }

            if let Some(ref profile) = post.profile {
                notes.push(format!("Profile/Company: {profile}"));
            }

            // LinkedIn is harder to verify without API access
            notes.push("Note: LinkedIn requires login for full verification".to_string());
            *needs_manual_review = true;

            Ok(SocialVerificationResult {
                platform: SocialPlatform::LinkedIn,
                is_valid_url: true,
                status: VerificationStatus::Verified,
                confidence,
                details: SocialDetails::LinkedIn(post),
                notes: notes.clone(),
                needs_manual_review: *needs_manual_review,
            })
        } else {
            notes.push("Could not parse LinkedIn URL".to_string());
            Ok(SocialVerificationResult {
                platform: SocialPlatform::LinkedIn,
                is_valid_url: false,
                status: VerificationStatus::Unverified,
                confidence: 0,
                details: SocialDetails::None,
                notes: notes.clone(),
                needs_manual_review: true,
            })
        }
    }

    /// Batch verify multiple URLs
    pub fn verify_urls(&self, urls: &[&str]) -> Vec<Result<SocialVerificationResult>> {
        let mut results = Vec::with_capacity(urls.len());
        for url in urls {
            results.push(self.verify_url(url));
        }
        results
    }

    /// Get a summary of verification results
    #[must_use]
    pub fn summarize_results(results: &[SocialVerificationResult]) -> VerificationSummary {
        let total = results.len();
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

/// Summary of verification results
#[derive(Debug, Clone, Serialize)]
pub struct VerificationSummary {
    /// Total URLs processed
    pub total_urls: usize,
    /// URLs that were verified
    pub verified_count: usize,
    /// URLs needing manual review
    pub needs_manual_review: usize,
    /// Average confidence score
    pub average_confidence: u32,
    /// Platforms found
    pub platforms_found: std::collections::HashMap<SocialPlatform, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
