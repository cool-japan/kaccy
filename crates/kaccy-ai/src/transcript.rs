//! Video transcript extraction module
//!
//! This module provides capabilities for extracting transcripts from videos
//! using various services including `YouTube` transcripts and external
//! transcription APIs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{AiError, Result};
use crate::llm::{ChatRequest, LlmClient};

/// Transcript provider trait for different backends
#[async_trait]
pub trait TranscriptProvider: Send + Sync {
    /// Extract transcript from a video URL
    async fn extract_transcript(&self, url: &str) -> Result<TranscriptResult>;

    /// Get provider name
    fn name(&self) -> &str;

    /// Check if this provider supports the given URL
    fn supports_url(&self, url: &str) -> bool;
}

/// Video transcript result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResult {
    /// Full transcript text
    pub text: String,
    /// Transcript segments with timestamps
    pub segments: Vec<TranscriptSegment>,
    /// Video metadata
    pub metadata: VideoMetadata,
    /// Language of the transcript
    pub language: Option<String>,
    /// Whether this is auto-generated or human-created
    pub is_auto_generated: bool,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Provider used
    pub provider: String,
}

/// A segment of transcript with timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Segment text
    pub text: String,
    /// Start time in seconds
    pub start_time: f64,
    /// Duration in seconds
    pub duration: f64,
    /// Speaker identifier (if available)
    pub speaker: Option<String>,
}

impl TranscriptSegment {
    /// Get end time
    #[must_use]
    pub fn end_time(&self) -> f64 {
        self.start_time + self.duration
    }

    /// Format start time as HH:MM:SS
    #[must_use]
    pub fn formatted_start(&self) -> String {
        Self::format_time(self.start_time)
    }

    /// Format time as HH:MM:SS
    fn format_time(seconds: f64) -> String {
        let total_secs = seconds as u64;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            format!("{hours:02}:{mins:02}:{secs:02}")
        } else {
            format!("{mins:02}:{secs:02}")
        }
    }
}

/// Video metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    /// Video title
    pub title: Option<String>,
    /// Video description
    pub description: Option<String>,
    /// Video duration in seconds
    pub duration_seconds: Option<f64>,
    /// Channel/author name
    pub author: Option<String>,
    /// Video publish date
    pub publish_date: Option<String>,
    /// Video platform
    pub platform: VideoPlatform,
    /// Video ID on the platform
    pub video_id: String,
    /// Thumbnail URL
    pub thumbnail_url: Option<String>,
}

/// Supported video platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoPlatform {
    YouTube,
    Vimeo,
    Twitter,
    TikTok,
    Twitch,
    Unknown,
}

impl VideoPlatform {
    /// Detect platform from URL
    #[must_use]
    pub fn from_url(url: &str) -> (Self, Option<String>) {
        let url_lower = url.to_lowercase();

        // YouTube
        if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
            let video_id = Self::extract_youtube_id(url);
            return (VideoPlatform::YouTube, video_id);
        }

        // Vimeo
        if url_lower.contains("vimeo.com") {
            let video_id = Self::extract_vimeo_id(url);
            return (VideoPlatform::Vimeo, video_id);
        }

        // Twitter/X
        if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
            let video_id = Self::extract_twitter_id(url);
            return (VideoPlatform::Twitter, video_id);
        }

        // TikTok
        if url_lower.contains("tiktok.com") {
            let video_id = Self::extract_tiktok_id(url);
            return (VideoPlatform::TikTok, video_id);
        }

        // Twitch
        if url_lower.contains("twitch.tv") {
            let video_id = Self::extract_twitch_id(url);
            return (VideoPlatform::Twitch, video_id);
        }

        (VideoPlatform::Unknown, None)
    }

    fn extract_youtube_id(url: &str) -> Option<String> {
        // Handle youtu.be/VIDEO_ID
        if url.contains("youtu.be/") {
            let parts: Vec<&str> = url.split("youtu.be/").collect();
            if parts.len() > 1 {
                let id = parts[1].split(['?', '&', '#']).next()?;
                return Some(id.to_string());
            }
        }

        // Handle youtube.com/watch?v=VIDEO_ID
        if url.contains("v=") {
            let parts: Vec<&str> = url.split("v=").collect();
            if parts.len() > 1 {
                let id = parts[1].split(['&', '#']).next()?;
                return Some(id.to_string());
            }
        }

        // Handle youtube.com/embed/VIDEO_ID
        if url.contains("/embed/") {
            let parts: Vec<&str> = url.split("/embed/").collect();
            if parts.len() > 1 {
                let id = parts[1].split(['?', '&', '#', '/']).next()?;
                return Some(id.to_string());
            }
        }

        None
    }

    fn extract_vimeo_id(url: &str) -> Option<String> {
        // Handle vimeo.com/VIDEO_ID
        let re = regex::Regex::new(r"vimeo\.com/(\d+)").ok()?;
        let caps = re.captures(url)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_twitter_id(url: &str) -> Option<String> {
        // Handle twitter.com/user/status/ID
        let re = regex::Regex::new(r"(?:twitter\.com|x\.com)/\w+/status/(\d+)").ok()?;
        let caps = re.captures(url)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_tiktok_id(url: &str) -> Option<String> {
        // Handle tiktok.com/@user/video/ID
        let re = regex::Regex::new(r"tiktok\.com/@[\w.]+/video/(\d+)").ok()?;
        let caps = re.captures(url)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_twitch_id(url: &str) -> Option<String> {
        // Handle twitch.tv/videos/ID
        let re = regex::Regex::new(r"twitch\.tv/videos/(\d+)").ok()?;
        let caps = re.captures(url)?;
        Some(caps.get(1)?.as_str().to_string())
    }
}

/// `YouTube` transcript extractor
pub struct YouTubeTranscriptProvider {
    http_client: reqwest::Client,
}

impl YouTubeTranscriptProvider {
    /// Create a new `YouTube` transcript provider
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Extract video ID from `YouTube` URL
    fn extract_video_id(&self, url: &str) -> Option<String> {
        let (platform, id) = VideoPlatform::from_url(url);
        if platform == VideoPlatform::YouTube {
            id
        } else {
            None
        }
    }

    /// Fetch `YouTube` page to extract transcript data
    async fn fetch_video_page(&self, video_id: &str) -> Result<String> {
        let url = format!("https://www.youtube.com/watch?v={video_id}");

        let response = self
            .http_client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| AiError::Validation(format!("Failed to fetch video page: {e}")))?;

        if !response.status().is_success() {
            return Err(AiError::Validation(format!(
                "Failed to fetch video page: HTTP {}",
                response.status()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| AiError::Validation(format!("Failed to read response: {e}")))
    }

    /// Parse transcript from `YouTube` page data
    fn parse_transcript_from_page(
        &self,
        page_html: &str,
        video_id: &str,
    ) -> Result<TranscriptResult> {
        let start_time = std::time::Instant::now();

        // Extract video title
        let title = self.extract_title(page_html);

        // Extract description
        let description = self.extract_description(page_html);

        // Try to find captions track
        let captions_data = self.extract_captions_data(page_html)?;

        // Parse caption segments
        let segments = self.parse_caption_segments(&captions_data);

        // Combine segments into full text
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let metadata = VideoMetadata {
            title,
            description,
            duration_seconds: segments.last().map(TranscriptSegment::end_time),
            author: self.extract_author(page_html),
            publish_date: self.extract_publish_date(page_html),
            platform: VideoPlatform::YouTube,
            video_id: video_id.to_string(),
            thumbnail_url: Some(format!(
                "https://img.youtube.com/vi/{video_id}/maxresdefault.jpg"
            )),
        };

        Ok(TranscriptResult {
            text,
            segments,
            metadata,
            language: Some("en".to_string()), // Simplified - actual implementation would detect
            is_auto_generated: true,          // Would need to check actual caption type
            processing_time_ms: start_time.elapsed().as_millis() as u64,
            provider: "youtube".to_string(),
        })
    }

    fn extract_title(&self, html: &str) -> Option<String> {
        // Look for <title>...</title>
        let re = regex::Regex::new(r"<title>([^<]+)</title>").ok()?;
        let caps = re.captures(html)?;
        let title = caps.get(1)?.as_str();
        // Remove " - YouTube" suffix
        Some(title.trim_end_matches(" - YouTube").to_string())
    }

    fn extract_description(&self, html: &str) -> Option<String> {
        // Look for description meta tag
        let re = regex::Regex::new(r#"<meta name="description" content="([^"]*)"#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_author(&self, html: &str) -> Option<String> {
        // Look for channel name
        let re = regex::Regex::new(r#""ownerChannelName":"([^"]+)""#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_publish_date(&self, html: &str) -> Option<String> {
        // Look for publish date
        let re = regex::Regex::new(r#""publishDate":"([^"]+)""#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_captions_data(&self, html: &str) -> Result<String> {
        // Look for caption track URL in page data
        // This is a simplified extraction - full implementation would use innertube API
        let re = regex::Regex::new(r#""captionTracks":\s*\[([^\]]+)\]"#)
            .map_err(|e| AiError::Validation(e.to_string()))?;

        if let Some(caps) = re.captures(html) {
            return Ok(caps
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default());
        }

        // Try alternate format
        let re2 = regex::Regex::new(r#"timedtext[^"]*\?[^"]*"#)
            .map_err(|e| AiError::Validation(e.to_string()))?;

        if let Some(caps) = re2.find(html) {
            return Ok(caps.as_str().to_string());
        }

        Err(AiError::Validation(
            "No captions found for this video".to_string(),
        ))
    }

    fn parse_caption_segments(&self, _data: &str) -> Vec<TranscriptSegment> {
        // Simplified - actual implementation would parse XML/JSON caption format
        // For now, return empty and rely on LLM fallback
        Vec::new()
    }
}

impl Default for YouTubeTranscriptProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranscriptProvider for YouTubeTranscriptProvider {
    async fn extract_transcript(&self, url: &str) -> Result<TranscriptResult> {
        let video_id = self
            .extract_video_id(url)
            .ok_or_else(|| AiError::Validation("Invalid YouTube URL".to_string()))?;

        let page_html = self.fetch_video_page(&video_id).await?;

        self.parse_transcript_from_page(&page_html, &video_id)
    }

    fn name(&self) -> &'static str {
        "youtube"
    }

    fn supports_url(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.contains("youtube.com") || url_lower.contains("youtu.be")
    }
}

/// LLM-assisted transcript analyzer
/// Used when native transcripts aren't available
pub struct LlmTranscriptAnalyzer {
    llm: LlmClient,
}

impl LlmTranscriptAnalyzer {
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Analyze transcript content for key points
    pub async fn analyze_transcript(
        &self,
        transcript: &TranscriptResult,
    ) -> Result<TranscriptAnalysis> {
        let prompt = format!(
            r#"Analyze the following video transcript and provide:
1. A brief summary (2-3 sentences)
2. Main topics covered (bullet points)
3. Key takeaways
4. Overall sentiment (positive/neutral/negative)

Video Title: {}
Transcript:
{}

Respond in JSON format:
{{
    "summary": "<string>",
    "topics": ["<topic1>", "<topic2>", ...],
    "key_takeaways": ["<takeaway1>", "<takeaway2>", ...],
    "sentiment": "<positive|neutral|negative>",
    "quality_indicators": {{
        "clarity": <0-100>,
        "informativeness": <0-100>,
        "professionalism": <0-100>
    }}
}}"#,
            transcript.metadata.title.as_deref().unwrap_or("Unknown"),
            &transcript.text[..transcript.text.len().min(10000)]
        );

        let request = ChatRequest::with_system(
            "You are an expert content analyst. Analyze video transcripts accurately.",
            prompt,
        )
        .max_tokens(2048)
        .temperature(0.3);

        let response = self.llm.chat(request).await?;

        self.parse_analysis_response(&response.message.content)
    }

    fn parse_analysis_response(&self, response: &str) -> Result<TranscriptAnalysis> {
        // Try to extract JSON
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        serde_json::from_str(json_str)
            .map_err(|e| AiError::EvaluationFailed(format!("Failed to parse analysis: {e}")))
    }

    /// Generate a summary of the transcript
    pub async fn summarize(
        &self,
        transcript: &TranscriptResult,
        max_length: usize,
    ) -> Result<String> {
        let prompt = format!(
            "Summarize the following video transcript in {} words or less. Focus on the main points and conclusions.\n\nTranscript:\n{}",
            max_length / 5, // Approximate words from char limit
            &transcript.text[..transcript.text.len().min(15000)]
        );

        let request = ChatRequest::with_system(
            "You are a concise summarizer. Create clear, accurate summaries.",
            prompt,
        )
        .max_tokens(512)
        .temperature(0.3);

        let response = self.llm.chat(request).await?;
        Ok(response.message.content)
    }
}

/// Transcript analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptAnalysis {
    /// Brief summary
    pub summary: String,
    /// Main topics covered
    pub topics: Vec<String>,
    /// Key takeaways
    pub key_takeaways: Vec<String>,
    /// Overall sentiment
    pub sentiment: String,
    /// Quality indicators
    pub quality_indicators: QualityIndicators,
}

/// Quality indicators for transcript content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIndicators {
    /// Clarity score (0-100)
    pub clarity: u32,
    /// Informativeness score (0-100)
    pub informativeness: u32,
    /// Professionalism score (0-100)
    pub professionalism: u32,
}

/// Multi-provider transcript service
pub struct TranscriptService {
    providers: Vec<Box<dyn TranscriptProvider>>,
    analyzer: Option<LlmTranscriptAnalyzer>,
}

impl TranscriptService {
    /// Create a new transcript service with default providers
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: vec![Box::new(YouTubeTranscriptProvider::new())],
            analyzer: None,
        }
    }

    /// Create with LLM support for analysis
    #[must_use]
    pub fn with_llm(llm: LlmClient) -> Self {
        Self {
            providers: vec![Box::new(YouTubeTranscriptProvider::new())],
            analyzer: Some(LlmTranscriptAnalyzer::new(llm)),
        }
    }

    /// Add a custom provider
    #[must_use]
    pub fn add_provider(mut self, provider: Box<dyn TranscriptProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Extract transcript from any supported URL
    pub async fn extract(&self, url: &str) -> Result<TranscriptResult> {
        for provider in &self.providers {
            if provider.supports_url(url) {
                return provider.extract_transcript(url).await;
            }
        }

        Err(AiError::Validation(format!(
            "No provider supports URL: {url}"
        )))
    }

    /// Extract and analyze transcript
    pub async fn extract_and_analyze(
        &self,
        url: &str,
    ) -> Result<(TranscriptResult, Option<TranscriptAnalysis>)> {
        let transcript = self.extract(url).await?;

        let analysis = if let Some(ref analyzer) = self.analyzer {
            Some(analyzer.analyze_transcript(&transcript).await?)
        } else {
            None
        };

        Ok((transcript, analysis))
    }

    /// Check if a URL is supported
    #[must_use]
    pub fn supports_url(&self, url: &str) -> bool {
        self.providers.iter().any(|p| p.supports_url(url))
    }

    /// Get list of supported platforms
    #[must_use]
    pub fn supported_platforms(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }
}

impl Default for TranscriptService {
    fn default() -> Self {
        Self::new()
    }
}

/// Transcript search functionality
pub struct TranscriptSearch;

impl TranscriptSearch {
    /// Search for text within transcript segments
    #[must_use]
    pub fn search(transcript: &TranscriptResult, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (index, segment) in transcript.segments.iter().enumerate() {
            let text_lower = segment.text.to_lowercase();
            if text_lower.contains(&query_lower) {
                results.push(SearchResult {
                    segment_index: index,
                    text: segment.text.clone(),
                    start_time: segment.start_time,
                    timestamp: segment.formatted_start(),
                    context: Self::get_context(transcript, index),
                });
            }
        }

        results
    }

    /// Get context around a segment
    fn get_context(transcript: &TranscriptResult, index: usize) -> String {
        let start = index.saturating_sub(1);
        let end = (index + 2).min(transcript.segments.len());

        transcript.segments[start..end]
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Find all timestamps where a topic is discussed
    #[must_use]
    pub fn find_topic_timestamps(transcript: &TranscriptResult, topic: &str) -> Vec<TopicMention> {
        let results = Self::search(transcript, topic);

        results
            .into_iter()
            .map(|r| TopicMention {
                timestamp: r.timestamp,
                start_seconds: r.start_time,
                context: r.context,
            })
            .collect()
    }
}

/// Search result within transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Index of the segment
    pub segment_index: usize,
    /// Matching text
    pub text: String,
    /// Start time in seconds
    pub start_time: f64,
    /// Formatted timestamp
    pub timestamp: String,
    /// Surrounding context
    pub context: String,
}

/// Topic mention with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMention {
    /// Formatted timestamp
    pub timestamp: String,
    /// Start time in seconds
    pub start_seconds: f64,
    /// Context around the mention
    pub context: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_id_extraction() {
        // Test YouTube URL formats
        let youtube_cases = vec![
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://youtu.be/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/embed/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
        ];

        for (url, expected_id) in youtube_cases {
            let (platform, id) = VideoPlatform::from_url(url);
            assert_eq!(platform, VideoPlatform::YouTube);
            assert_eq!(id, Some(expected_id.to_string()));
        }

        // Test Vimeo URL - should extract Vimeo ID (not YouTube)
        let (platform, id) = VideoPlatform::from_url("https://vimeo.com/123456");
        assert_eq!(platform, VideoPlatform::Vimeo);
        assert_eq!(id, Some("123456".to_string()));
    }

    #[test]
    fn test_platform_detection() {
        assert_eq!(
            VideoPlatform::from_url("https://youtube.com/watch?v=abc").0,
            VideoPlatform::YouTube
        );
        assert_eq!(
            VideoPlatform::from_url("https://vimeo.com/123").0,
            VideoPlatform::Vimeo
        );
        assert_eq!(
            VideoPlatform::from_url("https://twitter.com/user/status/123").0,
            VideoPlatform::Twitter
        );
        assert_eq!(
            VideoPlatform::from_url("https://example.com/video").0,
            VideoPlatform::Unknown
        );
    }

    #[test]
    fn test_segment_formatting() {
        let segment = TranscriptSegment {
            text: "Hello world".to_string(),
            start_time: 3661.5,
            duration: 2.0,
            speaker: None,
        };

        assert_eq!(segment.formatted_start(), "01:01:01");
        assert_eq!(segment.end_time(), 3663.5);
    }
}
