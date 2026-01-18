//! Optical Character Recognition (OCR) module
//!
//! This module provides OCR capabilities for extracting text from images
//! and screenshots. It supports multiple OCR backends including cloud
//! services and local processing.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{AiError, Result};
use crate::llm::{ChatRequest, LlmClient};

/// OCR provider trait for different backends
#[async_trait]
pub trait OcrProvider: Send + Sync {
    /// Extract text from image bytes
    async fn extract_text(&self, image_data: &[u8], format: ImageFormat) -> Result<OcrResult>;

    /// Extract text from image URL
    async fn extract_text_from_url(&self, url: &str) -> Result<OcrResult>;

    /// Get provider name
    fn name(&self) -> &str;
}

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// JPEG image
    Jpeg,
    /// PNG image
    Png,
    /// WebP image
    WebP,
    /// GIF image (first frame)
    Gif,
    /// BMP image
    Bmp,
    /// TIFF image
    Tiff,
    /// Unknown format (auto-detect)
    Unknown,
}

impl ImageFormat {
    /// Detect image format from magic bytes
    #[must_use]
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 2 {
            return ImageFormat::Unknown;
        }

        // Check magic bytes (ordered by minimum required length)
        // BMP: 2 bytes
        if data.starts_with(&[0x42, 0x4D]) {
            return ImageFormat::Bmp;
        }
        // JPEG: 3 bytes
        if data.len() >= 3 && data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return ImageFormat::Jpeg;
        }
        // TIFF: 4 bytes
        if data.len() >= 4
            && (data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
                || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]))
        {
            return ImageFormat::Tiff;
        }
        // GIF: 6 bytes
        if data.len() >= 6 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
            return ImageFormat::Gif;
        }
        // PNG: 8 bytes
        if data.len() >= 8 && data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return ImageFormat::Png;
        }
        // WebP: 12 bytes
        if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            return ImageFormat::WebP;
        }

        ImageFormat::Unknown
    }

    /// Detect format from file extension
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            "webp" => ImageFormat::WebP,
            "gif" => ImageFormat::Gif,
            "bmp" => ImageFormat::Bmp,
            "tif" | "tiff" => ImageFormat::Tiff,
            _ => ImageFormat::Unknown,
        }
    }

    /// Get MIME type for format
    #[must_use]
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Unknown => "application/octet-stream",
        }
    }
}

/// OCR extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Extracted text content
    pub text: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Language detected (ISO 639-1 code)
    pub language: Option<String>,
    /// Text blocks with positions
    pub blocks: Vec<TextBlock>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Provider used for extraction
    pub provider: String,
}

/// Text block with position information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// Text content
    pub text: String,
    /// Confidence for this block
    pub confidence: f64,
    /// Bounding box (x, y, width, height) as percentage of image
    pub bounding_box: Option<BoundingBox>,
    /// Block type (paragraph, line, word, etc.)
    pub block_type: BlockType,
}

/// Bounding box coordinates (normalized 0.0-1.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    /// X coordinate (left)
    pub x: f64,
    /// Y coordinate (top)
    pub y: f64,
    /// Width
    pub width: f64,
    /// Height
    pub height: f64,
}

/// Type of text block
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    /// Full page
    Page,
    /// Paragraph
    Paragraph,
    /// Single line
    Line,
    /// Single word
    Word,
    /// Symbol/character
    Symbol,
}

/// LLM-based OCR provider using vision models
pub struct LlmOcrProvider {
    llm: LlmClient,
    config: LlmOcrConfig,
}

/// Configuration for LLM OCR
#[derive(Debug, Clone)]
pub struct LlmOcrConfig {
    /// Maximum image size in bytes
    pub max_image_size: usize,
    /// Whether to include detailed analysis
    pub detailed_analysis: bool,
    /// Target language for extraction (optional)
    pub target_language: Option<String>,
}

impl Default for LlmOcrConfig {
    fn default() -> Self {
        Self {
            max_image_size: 20 * 1024 * 1024, // 20MB
            detailed_analysis: true,
            target_language: None,
        }
    }
}

impl LlmOcrProvider {
    /// Create a new LLM OCR provider
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self {
            llm,
            config: LlmOcrConfig::default(),
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(llm: LlmClient, config: LlmOcrConfig) -> Self {
        Self { llm, config }
    }

    fn build_ocr_prompt(&self, detailed: bool) -> String {
        if detailed {
            r#"Extract all text visible in this image. Please provide:

1. **Full Text**: All text content exactly as it appears
2. **Structure**: Identify headings, paragraphs, lists, tables if present
3. **Layout**: Note any important layout information

Respond in JSON format:
{
    "text": "<all extracted text>",
    "blocks": [
        {"text": "<block text>", "type": "<paragraph|heading|list|table|other>"}
    ],
    "language": "<detected language code or null>",
    "confidence": <0.0-1.0 confidence estimate>
}"#
            .to_string()
        } else {
            "Extract all text from this image. Return only the text content, preserving the layout as much as possible.".to_string()
        }
    }

    fn parse_ocr_response(&self, response: &str, detailed: bool) -> Result<OcrResult> {
        let start_time = std::time::Instant::now();

        if detailed {
            // Try to parse JSON response
            if let Some(start) = response.find('{') {
                if let Some(end) = response.rfind('}') {
                    let json_str = &response[start..=end];
                    if let Ok(parsed) = serde_json::from_str::<LlmOcrResponse>(json_str) {
                        return Ok(OcrResult {
                            text: parsed.text,
                            confidence: parsed.confidence.unwrap_or(0.8),
                            language: parsed.language,
                            blocks: parsed
                                .blocks
                                .unwrap_or_default()
                                .into_iter()
                                .map(|b| TextBlock {
                                    text: b.text,
                                    confidence: 0.8,
                                    bounding_box: None,
                                    block_type: match b.block_type.as_str() {
                                        "heading" => BlockType::Line,
                                        "paragraph" => BlockType::Paragraph,
                                        "list" | "table" => BlockType::Paragraph,
                                        _ => BlockType::Paragraph,
                                    },
                                })
                                .collect(),
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                            provider: "llm".to_string(),
                        });
                    }
                }
            }
        }

        // Fall back to plain text response
        Ok(OcrResult {
            text: response.trim().to_string(),
            confidence: 0.7,
            language: None,
            blocks: vec![TextBlock {
                text: response.trim().to_string(),
                confidence: 0.7,
                bounding_box: None,
                block_type: BlockType::Page,
            }],
            processing_time_ms: start_time.elapsed().as_millis() as u64,
            provider: "llm".to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct LlmOcrResponse {
    text: String,
    blocks: Option<Vec<LlmOcrBlock>>,
    language: Option<String>,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct LlmOcrBlock {
    text: String,
    #[serde(rename = "type")]
    block_type: String,
}

#[async_trait]
impl OcrProvider for LlmOcrProvider {
    async fn extract_text(&self, image_data: &[u8], format: ImageFormat) -> Result<OcrResult> {
        // Check image size
        if image_data.len() > self.config.max_image_size {
            return Err(AiError::Validation(format!(
                "Image too large: {} bytes (max {} bytes)",
                image_data.len(),
                self.config.max_image_size
            )));
        }

        // Encode image as base64
        let base64_image =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image_data);

        let prompt = self.build_ocr_prompt(self.config.detailed_analysis);

        // Create vision request with image
        let image_url = format!("data:{};base64,{}", format.mime_type(), base64_image);

        let request = ChatRequest::with_vision(
            "You are an expert OCR system. Extract text accurately from images.",
            prompt,
            image_url,
        )
        .max_tokens(4096)
        .temperature(0.1);

        let response = self.llm.chat(request).await?;

        self.parse_ocr_response(&response.message.content, self.config.detailed_analysis)
    }

    async fn extract_text_from_url(&self, url: &str) -> Result<OcrResult> {
        let prompt = self.build_ocr_prompt(self.config.detailed_analysis);

        let request = ChatRequest::with_vision(
            "You are an expert OCR system. Extract text accurately from images.",
            prompt,
            url.to_string(),
        )
        .max_tokens(4096)
        .temperature(0.1);

        let response = self.llm.chat(request).await?;

        self.parse_ocr_response(&response.message.content, self.config.detailed_analysis)
    }

    fn name(&self) -> &'static str {
        "llm-vision"
    }
}

/// Simple image analyzer for basic image properties
pub struct ImageAnalyzer;

impl ImageAnalyzer {
    /// Get image dimensions from bytes
    pub fn get_dimensions(data: &[u8]) -> Result<(u32, u32)> {
        use image::GenericImageView;

        let img = image::load_from_memory(data)
            .map_err(|e| AiError::Validation(format!("Failed to load image: {e}")))?;

        Ok(img.dimensions())
    }

    /// Check if image is likely a screenshot (based on dimensions)
    #[must_use]
    pub fn is_likely_screenshot(width: u32, height: u32) -> bool {
        // Common screen resolutions
        let common_widths = [1920, 2560, 3840, 1366, 1440, 1536, 1280];
        let common_heights = [1080, 1440, 2160, 768, 900, 864, 720, 800];

        common_widths.contains(&width) || common_heights.contains(&height)
    }

    /// Analyze image content for text regions (simple heuristic)
    pub fn estimate_text_regions(data: &[u8]) -> Result<TextRegionEstimate> {
        use image::GenericImageView;

        let img = image::load_from_memory(data)
            .map_err(|e| AiError::Validation(format!("Failed to load image: {e}")))?;

        let (width, height) = img.dimensions();
        let grayscale = img.to_luma8();

        // Calculate contrast and edges (simplified)
        let mut high_contrast_pixels = 0u64;
        let total_pixels = u64::from(width * height);

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let center = i32::from(grayscale.get_pixel(x, y)[0]);
                let right = i32::from(grayscale.get_pixel(x + 1, y)[0]);
                let down = i32::from(grayscale.get_pixel(x, y + 1)[0]);

                // Simple edge detection
                if (center - right).abs() > 30 || (center - down).abs() > 30 {
                    high_contrast_pixels += 1;
                }
            }
        }

        let edge_ratio = high_contrast_pixels as f64 / total_pixels as f64;

        // Text-heavy images typically have moderate edge ratios
        let likely_has_text = edge_ratio > 0.05 && edge_ratio < 0.4;

        Ok(TextRegionEstimate {
            width,
            height,
            edge_ratio,
            likely_has_text,
            estimated_text_coverage: if likely_has_text {
                (edge_ratio * 2.0).min(1.0)
            } else {
                0.0
            },
        })
    }
}

/// Estimate of text regions in an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegionEstimate {
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Ratio of high-contrast edges
    pub edge_ratio: f64,
    /// Whether the image likely contains text
    pub likely_has_text: bool,
    /// Estimated text coverage (0.0-1.0)
    pub estimated_text_coverage: f64,
}

/// Screenshot-specific OCR service
pub struct ScreenshotOcr {
    provider: Box<dyn OcrProvider>,
}

impl ScreenshotOcr {
    /// Create with default LLM provider
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self {
            provider: Box::new(LlmOcrProvider::new(llm)),
        }
    }

    /// Create with custom provider
    #[must_use]
    pub fn with_provider(provider: Box<dyn OcrProvider>) -> Self {
        Self { provider }
    }

    /// Process a screenshot and extract text
    pub async fn process_screenshot(&self, data: &[u8]) -> Result<ScreenshotAnalysis> {
        let format = ImageFormat::detect(data);

        // Analyze image properties
        let (width, height) = ImageAnalyzer::get_dimensions(data)?;
        let is_screenshot = ImageAnalyzer::is_likely_screenshot(width, height);
        let text_estimate = ImageAnalyzer::estimate_text_regions(data)?;

        // Skip OCR if unlikely to have text
        if !text_estimate.likely_has_text {
            return Ok(ScreenshotAnalysis {
                ocr_result: None,
                is_screenshot,
                dimensions: (width, height),
                format,
                has_text: false,
                text_estimate,
            });
        }

        // Perform OCR
        let ocr_result = self.provider.extract_text(data, format).await?;
        let has_text = !ocr_result.text.trim().is_empty();

        Ok(ScreenshotAnalysis {
            ocr_result: Some(ocr_result),
            is_screenshot,
            dimensions: (width, height),
            format,
            has_text,
            text_estimate,
        })
    }

    /// Process screenshot from file path
    pub async fn process_file(&self, path: &Path) -> Result<ScreenshotAnalysis> {
        let data = std::fs::read(path)
            .map_err(|e| AiError::Validation(format!("Failed to read file: {e}")))?;
        self.process_screenshot(&data).await
    }

    /// Process screenshot from URL
    pub async fn process_url(&self, url: &str) -> Result<ScreenshotAnalysis> {
        let ocr_result = self.provider.extract_text_from_url(url).await?;
        let has_text = !ocr_result.text.trim().is_empty();

        Ok(ScreenshotAnalysis {
            ocr_result: Some(ocr_result),
            is_screenshot: true, // Assume URL screenshots are actual screenshots
            dimensions: (0, 0),  // Unknown without downloading
            format: ImageFormat::Unknown,
            has_text,
            text_estimate: TextRegionEstimate {
                width: 0,
                height: 0,
                edge_ratio: 0.0,
                likely_has_text: true,
                estimated_text_coverage: 0.0,
            },
        })
    }
}

/// Screenshot analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotAnalysis {
    /// OCR result (None if no text detected)
    pub ocr_result: Option<OcrResult>,
    /// Whether image appears to be a screenshot
    pub is_screenshot: bool,
    /// Image dimensions (width, height)
    pub dimensions: (u32, u32),
    /// Image format
    pub format: ImageFormat,
    /// Whether text was detected
    pub has_text: bool,
    /// Text region estimate
    pub text_estimate: TextRegionEstimate,
}

impl ScreenshotAnalysis {
    /// Get extracted text if available
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.ocr_result.as_ref().map(|r| r.text.as_str())
    }

    /// Get confidence score
    #[must_use]
    pub fn confidence(&self) -> f64 {
        self.ocr_result.as_ref().map_or(0.0, |r| r.confidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        // PNG magic bytes
        let png_data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageFormat::detect(&png_data), ImageFormat::Png);

        // JPEG magic bytes
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(ImageFormat::detect(&jpeg_data), ImageFormat::Jpeg);

        // Unknown
        let unknown_data = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(ImageFormat::detect(&unknown_data), ImageFormat::Unknown);
    }

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ImageFormat::from_extension("png"), ImageFormat::Png);
        assert_eq!(ImageFormat::from_extension("jpg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension("jpeg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension("webp"), ImageFormat::WebP);
        assert_eq!(ImageFormat::from_extension("txt"), ImageFormat::Unknown);
    }

    #[test]
    fn test_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }

    #[test]
    fn test_is_likely_screenshot() {
        assert!(ImageAnalyzer::is_likely_screenshot(1920, 1080));
        assert!(ImageAnalyzer::is_likely_screenshot(2560, 1440));
        assert!(!ImageAnalyzer::is_likely_screenshot(500, 500));
    }
}
