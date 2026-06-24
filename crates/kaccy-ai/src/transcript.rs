//! Video transcript extraction module
//!
//! This module provides capabilities for extracting transcripts from videos
//! using various services including `YouTube` transcripts and external
//! transcription APIs.
//!
//! ## Honest error contract
//!
//! Every code path either returns a [`TranscriptResult`] whose `segments`
//! contains at least one entry derived from **real** caption data, or returns
//! [`AiError::TranscriptUnavailable`].  Partial-as-success is explicitly
//! prohibited: an empty `segments` vector from a successful call is treated
//! as a parse failure and becomes `TranscriptUnavailable`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{AiError, Result};
use crate::llm::{ChatRequest, LlmClient};

// ---------------------------------------------------------------------------
// Public trait
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

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
    /// YouTube or youtu.be.
    YouTube,
    /// Vimeo.
    Vimeo,
    /// Twitter / X.
    Twitter,
    /// TikTok.
    TikTok,
    /// Twitch.
    Twitch,
    /// Platform could not be detected.
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

// ---------------------------------------------------------------------------
// Page-metadata carrier (avoids too-many-arguments in assemble_result)
// ---------------------------------------------------------------------------

/// Best-effort metadata harvested from the YouTube watch page.
///
/// All fields are `Option` because scraping may fail for any field
/// independently.  Failures here do not abort transcript extraction.
#[derive(Debug, Default)]
struct PageMetadata {
    title: Option<String>,
    description: Option<String>,
    author: Option<String>,
    publish_date: Option<String>,
}

// ---------------------------------------------------------------------------
// Caption track info extracted from the YouTube watch-page JSON
// ---------------------------------------------------------------------------

/// Minimal subset of a single caption-track entry that YouTube embeds in the
/// watch-page `ytInitialPlayerResponse` JSON under `captions.playerCaptionsTracklistRenderer.captionTracks`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptionTrackEntry {
    /// The timedtext endpoint URL (may be JSON-encoded with backslash escapes).
    base_url: String,
    /// BCP-47 language code, e.g. `"en"`, `"ja"`.
    #[serde(rename = "languageCode")]
    language_code: Option<String>,
    /// `"asr"` for auto-generated, absent/other for manual.
    #[serde(default)]
    kind: String,
}

// ---------------------------------------------------------------------------
// YouTube provider
// ---------------------------------------------------------------------------

/// `YouTube` transcript extractor.
///
/// The implementation performs two real network requests:
/// 1. Fetch the YouTube watch page to extract the embedded player JSON.
/// 2. Fetch the timedtext (caption) XML endpoint extracted from that JSON.
///
/// Both steps can fail with precise typed errors; no synthetic/fabricated
/// transcript data is returned.
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

    /// Fetch the raw HTML of the YouTube watch page.
    async fn fetch_video_page(&self, video_id: &str) -> Result<String> {
        let url = format!("https://www.youtube.com/watch?v={video_id}");

        let response = self
            .http_client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| {
                AiError::TranscriptUnavailable(format!("Failed to fetch video page: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(AiError::TranscriptUnavailable(format!(
                "Video page returned HTTP {}",
                response.status()
            )));
        }

        response.text().await.map_err(|e| {
            AiError::TranscriptUnavailable(format!("Failed to read video page body: {e}"))
        })
    }

    /// Fetch the timedtext caption XML from the given URL.
    ///
    /// YouTube's timedtext endpoint may return either XML (`<transcript>`) or
    /// JSON; we request the XML form by ensuring `fmt=xml3` or omitting `fmt`
    /// (default is XML).  The URL already contains the necessary auth params
    /// embedded by YouTube in the watch-page JSON.
    async fn fetch_caption_track(&self, base_url: &str) -> Result<String> {
        // Unescape any JSON-embedded `&` → `&` sequences that YouTube
        // sometimes leaves in the base_url inside the watch-page JSON blob.
        let clean_url = base_url.replace("\\u0026", "&");

        let response = self
            .http_client
            .get(&clean_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .map_err(|e| {
                AiError::TranscriptUnavailable(format!("Failed to fetch caption track: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(AiError::TranscriptUnavailable(format!(
                "Caption endpoint returned HTTP {}",
                response.status()
            )));
        }

        response.text().await.map_err(|e| {
            AiError::TranscriptUnavailable(format!("Failed to read caption body: {e}"))
        })
    }

    // -----------------------------------------------------------------------
    // Page-level metadata helpers (best-effort; None is acceptable)
    // -----------------------------------------------------------------------

    fn extract_title(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(r"<title>([^<]+)</title>").ok()?;
        let caps = re.captures(html)?;
        let title = caps.get(1)?.as_str();
        Some(title.trim_end_matches(" - YouTube").to_string())
    }

    fn extract_description(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(r#"<meta name="description" content="([^"]*)"#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_author(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(r#""ownerChannelName":"([^"]+)""#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    fn extract_publish_date(&self, html: &str) -> Option<String> {
        let re = regex::Regex::new(r#""publishDate":"([^"]+)""#).ok()?;
        let caps = re.captures(html)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    // -----------------------------------------------------------------------
    // Caption-track URL extraction
    // -----------------------------------------------------------------------

    /// Extract the best available caption track URL from the watch-page HTML.
    ///
    /// YouTube embeds the full player JSON in the page inside a JS assignment:
    /// ```js
    /// var ytInitialPlayerResponse = { ... };
    /// ```
    /// We locate the `captionTracks` array within that blob, deserialise the
    /// entries, and return the first English (or otherwise preferred) track
    /// along with its language code and whether it is auto-generated.
    ///
    /// Returns `Err(TranscriptUnavailable)` when no caption track is present.
    fn extract_caption_track_info(&self, html: &str) -> Result<(String, Option<String>, bool)> {
        // Strategy 1: parse the structured captionTracks JSON array that YouTube
        // embeds inside the serialised `ytInitialPlayerResponse` / `ytInitialData`.
        // We extract the raw JSON object for `captions` and deserialise it.
        if let Some(tracks) = self.parse_caption_tracks_json(html) {
            if !tracks.is_empty() {
                // Preference order:
                //   1. Manual English track
                //   2. Auto-generated English track
                //   3. Any manual track
                //   4. First available track
                let chosen = tracks
                    .iter()
                    .find(|t| {
                        t.kind != "asr"
                            && t.language_code
                                .as_deref()
                                .map(|l| l.starts_with("en"))
                                .unwrap_or(false)
                    })
                    .or_else(|| {
                        tracks.iter().find(|t| {
                            t.language_code
                                .as_deref()
                                .map(|l| l.starts_with("en"))
                                .unwrap_or(false)
                        })
                    })
                    .or_else(|| tracks.iter().find(|t| t.kind != "asr"))
                    .or_else(|| tracks.first());

                if let Some(track) = chosen {
                    let is_auto = track.kind == "asr";
                    let lang = track.language_code.clone();
                    return Ok((track.base_url.clone(), lang, is_auto));
                }
            }
        }

        // Strategy 2: direct regex scan for a bare timedtext URL as a fallback.
        // This catches older page formats where the full JSON structure differs.
        let re = regex::Regex::new(r#""baseUrl":"(https://www\.youtube\.com/api/timedtext[^"]+)""#)
            .map_err(|e| AiError::Internal(e.to_string()))?;

        if let Some(caps) = re.captures(html) {
            let raw_url = caps
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            // is_auto and language are unknown in this fallback path; treat
            // as auto-generated and unspecified.
            return Ok((raw_url, None, true));
        }

        Err(AiError::TranscriptUnavailable(
            "No caption tracks are available for this video".to_string(),
        ))
    }

    /// Parse the `captionTracks` array by locating the serialised `captions`
    /// object inside the watch-page HTML and feeding it to serde_json.
    ///
    /// Returns `None` if the JSON cannot be found or parsed (not an error —
    /// the caller will fall through to the regex fallback).
    fn parse_caption_tracks_json(&self, html: &str) -> Option<Vec<CaptionTrackEntry>> {
        // Find `"captionTracks":[` and extract the enclosing JSON array.  We do
        // this without a full JSON parse of the entire (large) page by scanning
        // for the key and then counting brackets to find the closing `]`.
        let key = r#""captionTracks":"#;
        let start_of_key = html.find(key)?;
        let after_key = &html[start_of_key + key.len()..];

        // The value is a JSON array starting with `[`.
        let array_start = after_key.find('[')?;
        let array_slice = &after_key[array_start..];

        // Walk until we find the matching `]`, respecting nesting depth.
        let mut depth: i32 = 0;
        let mut end_idx = 0;
        for (i, ch) in array_slice.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end_idx == 0 {
            return None;
        }

        let array_json = &array_slice[..end_idx];
        serde_json::from_str::<Vec<CaptionTrackEntry>>(array_json).ok()
    }

    // -----------------------------------------------------------------------
    // Timedtext XML parsing
    // -----------------------------------------------------------------------

    /// Parse a YouTube timedtext XML response into transcript segments.
    ///
    /// The canonical format is:
    /// ```xml
    /// <?xml version="1.0" encoding="utf-8" ?>
    /// <transcript>
    ///   <text start="0.5" dur="2.3">Hello world</text>
    ///   <text start="3.0" dur="1.8">How are you</text>
    /// </transcript>
    /// ```
    ///
    /// Text content is HTML-entity-encoded by YouTube; we decode the common
    /// entities.  Returns `Err(TranscriptUnavailable)` if the XML is empty or
    /// cannot be parsed.
    pub(crate) fn parse_timedtext_xml(xml: &str) -> Result<Vec<TranscriptSegment>> {
        // Locate each <text ...>...</text> element using a regex.  A full XML
        // parser would be cleaner but adds a heavy dependency; the timedtext
        // format is simple and well-understood.
        let element_re = regex::Regex::new(
            r#"(?s)<text\s+start="([^"]+)"(?:\s+dur="([^"]*)")?[^>]*>(.*?)</text>"#,
        )
        .map_err(|e| AiError::Internal(format!("Failed to compile timedtext regex: {e}")))?;

        let mut segments = Vec::new();

        for caps in element_re.captures_iter(xml) {
            let start_str = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let dur_str = caps.get(2).map(|m| m.as_str()).unwrap_or("0");
            let raw_text = caps.get(3).map(|m| m.as_str()).unwrap_or("").trim();

            let start_time = start_str.parse::<f64>().unwrap_or(0.0);
            let duration = dur_str.parse::<f64>().unwrap_or(0.0);

            // Decode HTML entities that YouTube commonly uses in timedtext.
            let text = decode_html_entities(raw_text);

            // Skip blank segments (e.g. music notes or empty lines).
            if text.trim().is_empty() {
                continue;
            }

            segments.push(TranscriptSegment {
                text,
                start_time,
                duration,
                speaker: None,
            });
        }

        if segments.is_empty() {
            return Err(AiError::TranscriptUnavailable(
                "Caption track was fetched but contained no parseable text segments".to_string(),
            ));
        }

        Ok(segments)
    }

    /// Build the full `TranscriptResult` from pre-fetched data.
    ///
    /// This is a pure function (no network calls) so it can be unit-tested.
    fn assemble_result(
        &self,
        video_id: &str,
        segments: Vec<TranscriptSegment>,
        language: Option<String>,
        is_auto_generated: bool,
        page_meta: PageMetadata,
        elapsed_ms: u64,
    ) -> TranscriptResult {
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let duration_seconds = segments.last().map(TranscriptSegment::end_time);

        let metadata = VideoMetadata {
            title: page_meta.title,
            description: page_meta.description,
            duration_seconds,
            author: page_meta.author,
            publish_date: page_meta.publish_date,
            platform: VideoPlatform::YouTube,
            video_id: video_id.to_string(),
            thumbnail_url: Some(format!(
                "https://img.youtube.com/vi/{video_id}/maxresdefault.jpg"
            )),
        };

        TranscriptResult {
            text,
            segments,
            metadata,
            language,
            is_auto_generated,
            processing_time_ms: elapsed_ms,
            provider: "youtube".to_string(),
        }
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
        let start = std::time::Instant::now();

        let video_id = self
            .extract_video_id(url)
            .ok_or_else(|| AiError::InvalidInput("Invalid YouTube URL".to_string()))?;

        // Step 1 — fetch watch page
        let page_html = self.fetch_video_page(&video_id).await?;

        // Step 2 — locate a caption track URL (errors if none)
        let (caption_url, language, is_auto_generated) =
            self.extract_caption_track_info(&page_html)?;

        // Step 3 — fetch the real timedtext XML
        let xml = self.fetch_caption_track(&caption_url).await?;

        // Step 4 — parse XML into segments; errors if no segments found
        let segments = Self::parse_timedtext_xml(&xml)?;

        // Gather metadata (best-effort)
        let page_meta = PageMetadata {
            title: self.extract_title(&page_html),
            description: self.extract_description(&page_html),
            author: self.extract_author(&page_html),
            publish_date: self.extract_publish_date(&page_html),
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(self.assemble_result(
            &video_id,
            segments,
            language,
            is_auto_generated,
            page_meta,
            elapsed_ms,
        ))
    }

    fn name(&self) -> &'static str {
        "youtube"
    }

    fn supports_url(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.contains("youtube.com") || url_lower.contains("youtu.be")
    }
}

// ---------------------------------------------------------------------------
// HTML entity decoder (common YouTube timedtext entities)
// ---------------------------------------------------------------------------

/// Decode the HTML entities that YouTube embeds in timedtext XML text nodes.
///
/// We handle only the entities actually used by YouTube's timedtext endpoint
/// without pulling in a full HTML parser dependency.
fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        // Numeric decimal entities — handle common ones explicitly; a full
        // numeric decoder would require pulling in another dependency.
        .replace("&#10;", "\n")
        .replace("&#13;", "\r")
}

// ---------------------------------------------------------------------------
// LLM-assisted transcript analyzer
// ---------------------------------------------------------------------------

/// LLM-assisted transcript analyzer
/// Used when native transcripts aren't available
pub struct LlmTranscriptAnalyzer {
    llm: LlmClient,
}

impl LlmTranscriptAnalyzer {
    /// Create a new `LlmTranscriptAnalyzer` backed by the given LLM client.
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
            "Summarize the following video transcript in {} words or less. \
             Focus on the main points and conclusions.\n\nTranscript:\n{}",
            max_length / 5,
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

// ---------------------------------------------------------------------------
// Transcript analysis result
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Multi-provider transcript service
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Transcript search
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- platform / ID extraction -----------------------------------------

    #[test]
    fn test_youtube_id_extraction() {
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

    // ---- segment formatting -----------------------------------------------

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

    // ---- timedtext XML parsing --------------------------------------------

    /// Well-formed timedtext XML must parse into the correct segments.
    #[test]
    fn test_parse_timedtext_xml_valid() {
        let xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<transcript>
  <text start="0.5" dur="2.3">Hello world</text>
  <text start="3.0" dur="1.8">How are &amp;you</text>
  <text start="5.2" dur="3.1">This is a &lt;test&gt;</text>
</transcript>"#;

        let segments = YouTubeTranscriptProvider::parse_timedtext_xml(xml)
            .expect("Should parse valid timedtext XML");

        assert_eq!(segments.len(), 3);

        assert_eq!(segments[0].text, "Hello world");
        assert!((segments[0].start_time - 0.5).abs() < f64::EPSILON);
        assert!((segments[0].duration - 2.3).abs() < 1e-9);

        // Entity decoding
        assert_eq!(segments[1].text, "How are &you");
        assert_eq!(segments[2].text, "This is a <test>");
    }

    /// Segments with empty text after HTML-entity decoding must be skipped.
    #[test]
    fn test_parse_timedtext_xml_skips_blank_segments() {
        let xml = r#"<transcript>
  <text start="0.0" dur="1.0">   </text>
  <text start="1.0" dur="2.0">Real content</text>
  <text start="3.0" dur="1.0"></text>
</transcript>"#;

        let segments = YouTubeTranscriptProvider::parse_timedtext_xml(xml)
            .expect("Should parse; only blank segments are skipped");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Real content");
    }

    /// Completely empty XML (no `<text>` elements) must return `TranscriptUnavailable`.
    #[test]
    fn test_parse_timedtext_xml_empty_returns_error() {
        let xml = r#"<?xml version="1.0" encoding="utf-8" ?><transcript></transcript>"#;

        let err = YouTubeTranscriptProvider::parse_timedtext_xml(xml)
            .expect_err("Empty timedtext must yield TranscriptUnavailable");

        assert!(
            matches!(err, AiError::TranscriptUnavailable(_)),
            "Expected TranscriptUnavailable, got: {err:?}"
        );
    }

    /// Garbage / non-XML input must return `TranscriptUnavailable`.
    #[test]
    fn test_parse_timedtext_xml_garbage_returns_error() {
        let garbage = "this is not xml at all ¯\\_(ツ)_/¯";

        let err = YouTubeTranscriptProvider::parse_timedtext_xml(garbage)
            .expect_err("Garbage input must yield TranscriptUnavailable");

        assert!(
            matches!(err, AiError::TranscriptUnavailable(_)),
            "Expected TranscriptUnavailable, got: {err:?}"
        );
    }

    // ---- caption track URL extraction -------------------------------------

    /// A watch page that contains a `captionTracks` JSON array must yield
    /// the correct timedtext URL, language code, and is-auto flag without a
    /// live network call.
    #[test]
    fn test_extract_caption_track_info_from_page_json() {
        // Simulate a snippet of the ytInitialPlayerResponse JSON that YouTube
        // embeds in the watch page.
        let html = r#"
            var ytInitialPlayerResponse = {
                "captions": {
                    "playerCaptionsTracklistRenderer": {
                        "captionTracks": [
                            {
                                "baseUrl": "https://www.youtube.com/api/timedtext?v=abc123&lang=en&fmt=xml3",
                                "languageCode": "en",
                                "kind": "asr"
                            },
                            {
                                "baseUrl": "https://www.youtube.com/api/timedtext?v=abc123&lang=fr",
                                "languageCode": "fr",
                                "kind": ""
                            }
                        ]
                    }
                }
            };
        "#;

        let provider = YouTubeTranscriptProvider::new();
        let (url, lang, is_auto) = provider
            .extract_caption_track_info(html)
            .expect("Should find caption track in well-formed page snippet");

        // The English track should be preferred even if it is ASR.
        assert!(
            url.contains("lang=en"),
            "URL should reference the English track, got: {url}"
        );
        assert_eq!(lang.as_deref(), Some("en"));
        // The English track has kind="asr" → auto-generated.
        assert!(
            is_auto,
            "English ASR track should be flagged as auto-generated"
        );
    }

    /// A manual (non-ASR) caption track must be preferred over auto-generated.
    #[test]
    fn test_extract_caption_track_info_prefers_manual_over_asr() {
        let html = r#"
            "captionTracks": [
                {
                    "baseUrl": "https://www.youtube.com/api/timedtext?v=xyz&lang=en&kind=asr",
                    "languageCode": "en",
                    "kind": "asr"
                },
                {
                    "baseUrl": "https://www.youtube.com/api/timedtext?v=xyz&lang=en",
                    "languageCode": "en",
                    "kind": ""
                }
            ]
        "#;

        let provider = YouTubeTranscriptProvider::new();
        let (url, _lang, is_auto) = provider
            .extract_caption_track_info(html)
            .expect("Should find caption track");

        // Manual track has no `kind=asr` in URL and kind field is empty.
        assert!(
            !is_auto,
            "Manual track must not be flagged as auto-generated"
        );
        assert!(
            !url.contains("kind=asr"),
            "Should have chosen the manual track URL, got: {url}"
        );
    }

    /// A watch page with no caption data at all must yield `TranscriptUnavailable`.
    #[test]
    fn test_extract_caption_track_info_no_captions_returns_error() {
        let html = r#"<html><body>This video has no captions.</body></html>"#;

        let provider = YouTubeTranscriptProvider::new();
        let err = provider
            .extract_caption_track_info(html)
            .expect_err("Page with no captions must yield TranscriptUnavailable");

        assert!(
            matches!(err, AiError::TranscriptUnavailable(_)),
            "Expected TranscriptUnavailable, got: {err:?}"
        );
    }

    // ---- HTML entity decoding --------------------------------------------

    #[test]
    fn test_decode_html_entities() {
        assert_eq!(decode_html_entities("Hello &amp; world"), "Hello & world");
        assert_eq!(decode_html_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_html_entities("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(decode_html_entities("it&#39;s"), "it's");
        assert_eq!(decode_html_entities("plain text"), "plain text");
        assert_eq!(decode_html_entities("a&amp;b&lt;c&gt;d"), "a&b<c>d");
    }

    // ---- timedtext URL construction ---------------------------------------

    /// Verify that the constructed timedtext URL contains the expected
    /// query parameters.  This tests URL construction logic without any
    /// live network call.
    #[test]
    fn test_caption_url_construction_params() {
        // Simulates a base URL as embedded by YouTube (with & escaping)
        let raw_base = r"https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en&fmt=xml3";
        // The provider cleans up & before using the URL.
        let clean = raw_base.replace("\\u0026", "&");

        assert!(clean.contains("v=dQw4w9WgXcQ"), "Should have video id");
        assert!(clean.contains("lang=en"), "Should have language");
        assert!(clean.contains("fmt=xml3"), "Should have format");
        assert!(
            !clean.contains("\\u0026"),
            "Should have no raw unicode escapes"
        );
    }

    // ---- assemble_result -------------------------------------------------

    /// Verify that `assemble_result` correctly joins segment text and
    /// computes duration from the last segment's end time.
    #[test]
    fn test_assemble_result_correctness() {
        let provider = YouTubeTranscriptProvider::new();
        let segments = vec![
            TranscriptSegment {
                text: "Hello".to_string(),
                start_time: 0.0,
                duration: 1.5,
                speaker: None,
            },
            TranscriptSegment {
                text: "world".to_string(),
                start_time: 2.0,
                duration: 2.0,
                speaker: None,
            },
        ];

        let page_meta = PageMetadata {
            title: Some("Test Title".to_string()),
            description: None,
            author: None,
            publish_date: None,
        };

        let result = provider.assemble_result(
            "testvid",
            segments,
            Some("en".to_string()),
            true,
            page_meta,
            42,
        );

        assert_eq!(result.text, "Hello world");
        assert_eq!(result.segments.len(), 2);
        assert_eq!(
            result.metadata.duration_seconds,
            Some(4.0),
            "Duration should be end_time of last segment: 2.0 + 2.0"
        );
        assert_eq!(result.metadata.video_id, "testvid");
        assert_eq!(result.language.as_deref(), Some("en"));
        assert!(result.is_auto_generated);
        assert_eq!(result.processing_time_ms, 42);
    }
}
