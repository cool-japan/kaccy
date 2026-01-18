//! Document analysis module
//!
//! This module provides utilities for parsing and analyzing documents
//! in various formats including Markdown, HTML, and PDF.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Supported document formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentFormat {
    /// Markdown document
    Markdown,
    /// HTML document
    Html,
    /// Plain text
    PlainText,
    /// PDF document
    Pdf,
}

impl DocumentFormat {
    /// Detect format from content
    #[must_use]
    pub fn detect(content: &str) -> Self {
        let content_lower = content.to_lowercase();

        // Check for HTML markers
        if content_lower.contains("<!doctype html")
            || content_lower.contains("<html")
            || (content_lower.contains("<head") && content_lower.contains("<body"))
            || content_lower.contains("<div")
            || content_lower.contains("<p>")
        {
            return DocumentFormat::Html;
        }

        // Check for Markdown markers
        if content.contains("# ")
            || content.contains("## ")
            || content.contains("```")
            || content.contains("**")
            || content.contains("__")
            || content.contains("](") // Markdown link pattern [text](url)
            || content.contains("![")
            || content.contains("- [ ]")
            || content.contains("- [x]")
        {
            return DocumentFormat::Markdown;
        }

        DocumentFormat::PlainText
    }

    /// Detect format from file extension
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" | "mdown" | "mkd" => DocumentFormat::Markdown,
            "html" | "htm" | "xhtml" => DocumentFormat::Html,
            "pdf" => DocumentFormat::Pdf,
            _ => DocumentFormat::PlainText,
        }
    }

    /// Detect format from binary data (for PDF detection)
    #[must_use]
    pub fn detect_from_bytes(data: &[u8]) -> Self {
        // Check for PDF magic bytes (%PDF-)
        if data.len() >= 5 && &data[0..5] == b"%PDF-" {
            return DocumentFormat::Pdf;
        }

        // Fall back to string-based detection
        if let Ok(content) = std::str::from_utf8(data) {
            Self::detect(content)
        } else {
            // Binary content that's not PDF
            DocumentFormat::PlainText
        }
    }
}

/// Extracted document structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStructure {
    /// Document format
    pub format: DocumentFormat,
    /// Title (if detected)
    pub title: Option<String>,
    /// Headings with their levels
    pub headings: Vec<Heading>,
    /// Extracted links
    pub links: Vec<Link>,
    /// Extracted images
    pub images: Vec<Image>,
    /// Code blocks
    pub code_blocks: Vec<CodeBlock>,
    /// Plain text content (HTML tags stripped)
    pub plain_text: String,
    /// Word count
    pub word_count: usize,
    /// Character count
    pub char_count: usize,
    /// Estimated reading time in minutes
    pub reading_time_minutes: u32,
    /// Key statistics
    pub stats: DocumentStats,
}

/// Document heading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    /// Heading level (1-6)
    pub level: u8,
    /// Heading text
    pub text: String,
    /// Anchor/ID (if available)
    pub anchor: Option<String>,
}

/// Extracted link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// Link URL
    pub url: String,
    /// Link text
    pub text: String,
    /// Link title (if available)
    pub title: Option<String>,
    /// Whether this is an external link
    pub is_external: bool,
}

/// Extracted image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Image URL/path
    pub src: String,
    /// Alt text
    pub alt: String,
    /// Title (if available)
    pub title: Option<String>,
}

/// Code block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    /// Programming language (if specified)
    pub language: Option<String>,
    /// Code content
    pub code: String,
    /// Line count
    pub line_count: usize,
}

/// Document statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentStats {
    /// Number of headings
    pub heading_count: usize,
    /// Number of paragraphs
    pub paragraph_count: usize,
    /// Number of lists
    pub list_count: usize,
    /// Number of links
    pub link_count: usize,
    /// Number of images
    pub image_count: usize,
    /// Number of code blocks
    pub code_block_count: usize,
    /// Number of tables
    pub table_count: usize,
    /// Number of blockquotes
    pub blockquote_count: usize,
}

/// Document parser
pub struct DocumentParser;

impl DocumentParser {
    /// Parse a document and extract its structure
    #[must_use]
    pub fn parse(content: &str) -> DocumentStructure {
        let format = DocumentFormat::detect(content);

        match format {
            DocumentFormat::Markdown => Self::parse_markdown(content),
            DocumentFormat::Html => Self::parse_html(content),
            DocumentFormat::PlainText => Self::parse_plain_text(content),
            DocumentFormat::Pdf => Self::parse_plain_text(content), // PDF needs binary data
        }
    }

    /// Parse a document with explicit format
    #[must_use]
    pub fn parse_with_format(content: &str, format: DocumentFormat) -> DocumentStructure {
        match format {
            DocumentFormat::Markdown => Self::parse_markdown(content),
            DocumentFormat::Html => Self::parse_html(content),
            DocumentFormat::PlainText => Self::parse_plain_text(content),
            DocumentFormat::Pdf => Self::parse_plain_text(content), // PDF needs binary data
        }
    }

    /// Parse binary PDF data and extract its structure
    pub fn parse_pdf(data: &[u8]) -> Result<DocumentStructure, PdfParseError> {
        PdfParser::parse(data)
    }

    /// Parse binary PDF from a file path
    pub fn parse_pdf_file(path: &std::path::Path) -> Result<DocumentStructure, PdfParseError> {
        let data = std::fs::read(path).map_err(|e| PdfParseError::IoError(e.to_string()))?;
        Self::parse_pdf(&data)
    }

    /// Parse Markdown document
    fn parse_markdown(content: &str) -> DocumentStructure {
        let mut headings = Vec::new();
        let mut links = Vec::new();
        let mut images = Vec::new();
        let mut code_blocks = Vec::new();
        let mut title = None;
        let mut stats = DocumentStats::default();

        let mut in_code_block = false;
        let mut code_block_lang = None;
        let mut code_block_content = String::new();

        for line in content.lines() {
            // Handle code blocks
            if line.starts_with("```") {
                if in_code_block {
                    // End of code block
                    code_blocks.push(CodeBlock {
                        language: code_block_lang.take(),
                        line_count: code_block_content.lines().count(),
                        code: std::mem::take(&mut code_block_content),
                    });
                    stats.code_block_count += 1;
                    in_code_block = false;
                } else {
                    // Start of code block
                    let lang = line.trim_start_matches("```").trim();
                    code_block_lang = if lang.is_empty() {
                        None
                    } else {
                        Some(lang.to_string())
                    };
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                code_block_content.push_str(line);
                code_block_content.push('\n');
                continue;
            }

            // Parse headings
            if let Some(heading) = Self::parse_markdown_heading(line) {
                if title.is_none() && heading.level == 1 {
                    title = Some(heading.text.clone());
                }
                headings.push(heading);
                stats.heading_count += 1;
            }

            // Parse links: [text](url) or [text](url "title")
            Self::extract_markdown_links(line, &mut links);

            // Parse images: ![alt](src) or ![alt](src "title")
            Self::extract_markdown_images(line, &mut images);

            // Count lists
            if line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("+ ")
                || line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                    && line.contains(". ")
            {
                stats.list_count += 1;
            }

            // Count blockquotes
            if line.trim_start().starts_with("> ") {
                stats.blockquote_count += 1;
            }

            // Count tables (simple detection)
            if line.contains('|') && line.trim().starts_with('|') {
                stats.table_count += 1;
            }
        }

        stats.link_count = links.len();
        stats.image_count = images.len();

        // Calculate plain text
        let plain_text = Self::markdown_to_plain_text(content);
        let word_count = plain_text.split_whitespace().count();
        let char_count = plain_text.chars().count();

        // Count paragraphs (blank line separated blocks)
        stats.paragraph_count = content
            .split("\n\n")
            .filter(|p| !p.trim().is_empty() && !p.trim().starts_with('#'))
            .count();

        DocumentStructure {
            format: DocumentFormat::Markdown,
            title,
            headings,
            links,
            images,
            code_blocks,
            plain_text,
            word_count,
            char_count,
            reading_time_minutes: (word_count / 200).max(1) as u32,
            stats,
        }
    }

    /// Parse a markdown heading
    fn parse_markdown_heading(line: &str) -> Option<Heading> {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            return None;
        }

        let mut level = 0u8;
        for c in trimmed.chars() {
            if c == '#' {
                level += 1;
            } else {
                break;
            }
        }

        if level > 6 {
            return None;
        }

        let text = trimmed.trim_start_matches('#').trim().to_string();
        if text.is_empty() {
            return None;
        }

        // Generate anchor from text
        let anchor = text
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>();

        Some(Heading {
            level,
            text,
            anchor: Some(anchor),
        })
    }

    /// Extract markdown links from a line
    fn extract_markdown_links(line: &str, links: &mut Vec<Link>) {
        let mut remaining = line;

        while let Some(start) = remaining.find('[') {
            let after_start = &remaining[start + 1..];

            // Find closing bracket
            if let Some(close) = after_start.find(']') {
                let text = &after_start[..close];
                let after_close = &after_start[close + 1..];

                // Check for (url) or (url "title")
                if after_close.starts_with('(') {
                    if let Some(paren_close) = after_close.find(')') {
                        let url_part = &after_close[1..paren_close];

                        // Parse URL and optional title
                        let (url, title) = if let Some(quote_start) = url_part.find('"') {
                            let url = url_part[..quote_start].trim().to_string();
                            let title_part = &url_part[quote_start + 1..];
                            let title = title_part.trim_end_matches('"').to_string();
                            (url, Some(title))
                        } else {
                            (url_part.trim().to_string(), None)
                        };

                        // Skip image links (they start with !)
                        if !remaining[..start].ends_with('!') && !url.is_empty() {
                            let is_external = url.starts_with("http://")
                                || url.starts_with("https://")
                                || url.starts_with("//");

                            links.push(Link {
                                url,
                                text: text.to_string(),
                                title,
                                is_external,
                            });
                        }

                        remaining = &after_close[paren_close + 1..];
                        continue;
                    }
                }
            }

            remaining = &remaining[start + 1..];
        }
    }

    /// Extract markdown images from a line
    fn extract_markdown_images(line: &str, images: &mut Vec<Image>) {
        let mut remaining = line;

        while let Some(start) = remaining.find("![") {
            let after_start = &remaining[start + 2..];

            // Find closing bracket
            if let Some(close) = after_start.find(']') {
                let alt = &after_start[..close];
                let after_close = &after_start[close + 1..];

                // Check for (src) or (src "title")
                if after_close.starts_with('(') {
                    if let Some(paren_close) = after_close.find(')') {
                        let src_part = &after_close[1..paren_close];

                        // Parse src and optional title
                        let (src, title) = if let Some(quote_start) = src_part.find('"') {
                            let src = src_part[..quote_start].trim().to_string();
                            let title_part = &src_part[quote_start + 1..];
                            let title = title_part.trim_end_matches('"').to_string();
                            (src, Some(title))
                        } else {
                            (src_part.trim().to_string(), None)
                        };

                        if !src.is_empty() {
                            images.push(Image {
                                src,
                                alt: alt.to_string(),
                                title,
                            });
                        }

                        remaining = &after_close[paren_close + 1..];
                        continue;
                    }
                }
            }

            remaining = &remaining[start + 2..];
        }
    }

    /// Convert markdown to plain text
    fn markdown_to_plain_text(content: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;

        for line in content.lines() {
            if line.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }

            if in_code_block {
                continue;
            }

            // Remove headings markers
            let line = if line.starts_with('#') {
                line.trim_start_matches('#').trim()
            } else {
                line
            };

            // Remove bold/italic markers
            let line = line
                .replace("**", "")
                .replace("__", "")
                .replace(['*', '_'], "");

            // Remove inline code
            let line = Self::remove_inline_code(&line);

            // Remove links but keep text
            let line = Self::remove_markdown_links(&line);

            // Remove images
            let line = Self::remove_markdown_images(&line);

            if !line.trim().is_empty() {
                result.push_str(&line);
                result.push(' ');
            }
        }

        result.trim().to_string()
    }

    /// Remove inline code markers
    fn remove_inline_code(line: &str) -> String {
        let mut result = String::new();
        let mut in_code = false;

        for c in line.chars() {
            if c == '`' {
                in_code = !in_code;
            } else if !in_code {
                result.push(c);
            }
        }

        result
    }

    /// Remove markdown links but keep text
    fn remove_markdown_links(line: &str) -> String {
        let mut result = line.to_string();

        // Simple replacement of [text](url) with text
        while let Some(start) = result.find('[') {
            if let Some(close) = result[start..].find(']') {
                let absolute_close = start + close;
                if result.len() > absolute_close + 1
                    && result.as_bytes()[absolute_close + 1] == b'('
                {
                    if let Some(paren_close) = result[absolute_close..].find(')') {
                        let text = &result[start + 1..absolute_close];
                        let before = &result[..start];
                        let after = &result[absolute_close + paren_close + 1..];
                        result = format!("{before}{text}{after}");
                        continue;
                    }
                }
            }
            break;
        }

        result
    }

    /// Remove markdown images
    fn remove_markdown_images(line: &str) -> String {
        let mut result = line.to_string();

        while let Some(start) = result.find("![") {
            if let Some(close) = result[start..].find(']') {
                let absolute_close = start + close;
                if result.len() > absolute_close + 1
                    && result.as_bytes()[absolute_close + 1] == b'('
                {
                    if let Some(paren_close) = result[absolute_close..].find(')') {
                        let before = &result[..start];
                        let after = &result[absolute_close + paren_close + 1..];
                        result = format!("{before}{after}");
                        continue;
                    }
                }
            }
            break;
        }

        result
    }

    /// Parse HTML document
    fn parse_html(content: &str) -> DocumentStructure {
        let mut headings = Vec::new();
        let mut links = Vec::new();
        let mut images = Vec::new();
        let mut code_blocks = Vec::new();
        let mut title = None;
        let mut stats = DocumentStats::default();

        // Extract title from <title> tag
        if let Some(title_text) = Self::extract_html_tag_content(content, "title") {
            title = Some(title_text);
        }

        // Extract headings (h1-h6)
        for level in 1..=6 {
            let tag = format!("h{level}");
            for text in Self::extract_all_html_tag_contents(content, &tag) {
                if title.is_none() && level == 1 {
                    title = Some(text.clone());
                }
                headings.push(Heading {
                    level: level as u8,
                    text,
                    anchor: None,
                });
                stats.heading_count += 1;
            }
        }

        // Extract links
        Self::extract_html_links(content, &mut links);
        stats.link_count = links.len();

        // Extract images
        Self::extract_html_images(content, &mut images);
        stats.image_count = images.len();

        // Extract code blocks (<pre><code> or <code>)
        for code in Self::extract_all_html_tag_contents(content, "code") {
            code_blocks.push(CodeBlock {
                language: None,
                line_count: code.lines().count(),
                code,
            });
            stats.code_block_count += 1;
        }

        // Count other elements
        stats.paragraph_count = Self::count_html_tags(content, "p");
        stats.list_count =
            Self::count_html_tags(content, "ul") + Self::count_html_tags(content, "ol");
        stats.table_count = Self::count_html_tags(content, "table");
        stats.blockquote_count = Self::count_html_tags(content, "blockquote");

        // Get plain text
        let plain_text = Self::html_to_plain_text(content);
        let word_count = plain_text.split_whitespace().count();
        let char_count = plain_text.chars().count();

        DocumentStructure {
            format: DocumentFormat::Html,
            title,
            headings,
            links,
            images,
            code_blocks,
            plain_text,
            word_count,
            char_count,
            reading_time_minutes: (word_count / 200).max(1) as u32,
            stats,
        }
    }

    /// Extract content from an HTML tag
    fn extract_html_tag_content(content: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{tag}");
        let close_tag = format!("</{tag}>");

        let start = content.to_lowercase().find(&open_tag)?;
        let after_open = &content[start..];

        // Find the end of the opening tag
        let tag_end = after_open.find('>')?;
        let content_start = start + tag_end + 1;

        let close_pos = content[content_start..].to_lowercase().find(&close_tag)?;

        let text = &content[content_start..content_start + close_pos];
        Some(Self::html_to_plain_text(text).trim().to_string())
    }

    /// Extract all contents from HTML tags
    fn extract_all_html_tag_contents(content: &str, tag: &str) -> Vec<String> {
        let mut results = Vec::new();
        let content_lower = content.to_lowercase();
        let open_tag = format!("<{tag}");
        let close_tag = format!("</{tag}>");

        let mut search_start = 0;
        while let Some(start) = content_lower[search_start..].find(&open_tag) {
            let absolute_start = search_start + start;
            let after_open = &content[absolute_start..];

            if let Some(tag_end) = after_open.find('>') {
                let content_start = absolute_start + tag_end + 1;

                if let Some(close_pos) = content_lower[content_start..].find(&close_tag) {
                    let text = &content[content_start..content_start + close_pos];
                    let clean_text = Self::html_to_plain_text(text).trim().to_string();
                    if !clean_text.is_empty() {
                        results.push(clean_text);
                    }
                    search_start = content_start + close_pos + close_tag.len();
                    continue;
                }
            }

            search_start = absolute_start + 1;
        }

        results
    }

    /// Count occurrences of an HTML tag
    fn count_html_tags(content: &str, tag: &str) -> usize {
        let open_tag = format!("<{tag}");
        content.to_lowercase().matches(&open_tag).count()
    }

    /// Extract HTML links
    fn extract_html_links(content: &str, links: &mut Vec<Link>) {
        let content_lower = content.to_lowercase();
        let mut search_start = 0;

        while let Some(start) = content_lower[search_start..].find("<a ") {
            let absolute_start = search_start + start;
            let after_open = &content[absolute_start..];

            if let Some(tag_end) = after_open.find('>') {
                let tag_content = &after_open[..tag_end];

                // Extract href
                if let Some(href) = Self::extract_html_attribute(tag_content, "href") {
                    let close_pos = content_lower[absolute_start..].find("</a>");

                    let text = if let Some(close) = close_pos {
                        let content_start = absolute_start + tag_end + 1;
                        let content_end = absolute_start + close;
                        Self::html_to_plain_text(&content[content_start..content_end])
                            .trim()
                            .to_string()
                    } else {
                        String::new()
                    };

                    let title = Self::extract_html_attribute(tag_content, "title");
                    let is_external = href.starts_with("http://")
                        || href.starts_with("https://")
                        || href.starts_with("//");

                    links.push(Link {
                        url: href,
                        text,
                        title,
                        is_external,
                    });
                }

                search_start = absolute_start + tag_end;
            } else {
                search_start = absolute_start + 1;
            }
        }
    }

    /// Extract HTML images
    fn extract_html_images(content: &str, images: &mut Vec<Image>) {
        let content_lower = content.to_lowercase();
        let mut search_start = 0;

        while let Some(start) = content_lower[search_start..].find("<img ") {
            let absolute_start = search_start + start;
            let after_open = &content[absolute_start..];

            if let Some(tag_end) = after_open.find('>').or_else(|| after_open.find("/>")) {
                let tag_content = &after_open[..tag_end];

                if let Some(src) = Self::extract_html_attribute(tag_content, "src") {
                    let alt = Self::extract_html_attribute(tag_content, "alt").unwrap_or_default();
                    let title = Self::extract_html_attribute(tag_content, "title");

                    images.push(Image { src, alt, title });
                }

                search_start = absolute_start + tag_end;
            } else {
                search_start = absolute_start + 1;
            }
        }
    }

    /// Extract an HTML attribute value
    fn extract_html_attribute(tag_content: &str, attr: &str) -> Option<String> {
        let attr_pattern = format!("{attr}=");
        let content_lower = tag_content.to_lowercase();

        let attr_start = content_lower.find(&attr_pattern)?;
        let after_attr = &tag_content[attr_start + attr_pattern.len()..];

        // Handle quoted attribute values
        let first_char = after_attr.chars().next()?;
        if first_char == '"' || first_char == '\'' {
            let quote = first_char;
            let value_start = 1;
            let value_end = after_attr[value_start..].find(quote)?;
            return Some(after_attr[value_start..value_start + value_end].to_string());
        }

        // Handle unquoted attribute values
        let value_end = after_attr.find(|c: char| c.is_whitespace() || c == '>')?;
        Some(after_attr[..value_end].to_string())
    }

    /// Convert HTML to plain text
    fn html_to_plain_text(content: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;

        let content_lower = content.to_lowercase();
        let chars: Vec<char> = content.chars().collect();
        let chars_lower: Vec<char> = content_lower.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            // Check for script/style tags
            if i + 7 < chars.len() {
                let slice: String = chars_lower[i..i + 7].iter().collect();
                if slice == "<script" {
                    in_script = true;
                } else if slice == "</scrip" {
                    in_script = false;
                }
            }

            if i + 6 < chars.len() {
                let slice: String = chars_lower[i..i + 6].iter().collect();
                if slice == "<style" {
                    in_style = true;
                } else if slice == "</styl" {
                    in_style = false;
                }
            }

            let c = chars[i];

            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
                // Add space after certain tags
                result.push(' ');
            } else if !in_tag && !in_script && !in_style {
                result.push(c);
            }

            i += 1;
        }

        // Decode common HTML entities
        let result = result
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#39;", "'");

        // Normalize whitespace
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Parse plain text document
    fn parse_plain_text(content: &str) -> DocumentStructure {
        let word_count = content.split_whitespace().count();
        let char_count = content.chars().count();
        let paragraph_count = content
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .count();

        DocumentStructure {
            format: DocumentFormat::PlainText,
            title: None,
            headings: Vec::new(),
            links: Vec::new(),
            images: Vec::new(),
            code_blocks: Vec::new(),
            plain_text: content.to_string(),
            word_count,
            char_count,
            reading_time_minutes: (word_count / 200).max(1) as u32,
            stats: DocumentStats {
                paragraph_count,
                ..Default::default()
            },
        }
    }
}

/// Document quality analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentQuality {
    /// Overall quality score (0-100)
    pub overall_score: u32,
    /// Readability score (0-100)
    pub readability_score: u32,
    /// Structure score (0-100)
    pub structure_score: u32,
    /// Issues found
    pub issues: Vec<QualityIssue>,
    /// Suggestions for improvement
    pub suggestions: Vec<String>,
}

/// Quality issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    /// Issue severity
    pub severity: IssueSeverity,
    /// Issue description
    pub description: String,
}

/// Issue severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

/// Document quality analyzer
pub struct QualityAnalyzer;

impl QualityAnalyzer {
    /// Analyze document quality
    #[must_use]
    pub fn analyze(structure: &DocumentStructure) -> DocumentQuality {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Check for title
        if structure.title.is_none() {
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                description: "Document has no title".to_string(),
            });
            suggestions
                .push("Add a main heading (# Title) at the start of the document".to_string());
        }

        // Check heading structure
        let mut prev_level = 0u8;
        for heading in &structure.headings {
            if heading.level > prev_level + 1 && prev_level > 0 {
                issues.push(QualityIssue {
                    severity: IssueSeverity::Warning,
                    description: format!(
                        "Heading level jumps from {} to {}: '{}'",
                        prev_level, heading.level, heading.text
                    ),
                });
            }
            prev_level = heading.level;
        }

        // Check word count
        if structure.word_count < 100 {
            issues.push(QualityIssue {
                severity: IssueSeverity::Info,
                description: "Document is very short".to_string(),
            });
        } else if structure.word_count > 5000 {
            suggestions.push("Consider breaking long documents into multiple sections".to_string());
        }

        // Check for broken/empty links
        for link in &structure.links {
            if link.url.is_empty() {
                issues.push(QualityIssue {
                    severity: IssueSeverity::Error,
                    description: format!("Empty link URL for text: '{}'", link.text),
                });
            }
            if link.text.is_empty() {
                issues.push(QualityIssue {
                    severity: IssueSeverity::Warning,
                    description: format!("Link has no text: '{}'", link.url),
                });
            }
        }

        // Check for images without alt text
        for image in &structure.images {
            if image.alt.is_empty() {
                issues.push(QualityIssue {
                    severity: IssueSeverity::Warning,
                    description: format!("Image missing alt text: '{}'", image.src),
                });
            }
        }

        // Calculate scores
        let structure_score = Self::calculate_structure_score(structure, &issues);
        let readability_score = Self::calculate_readability_score(structure);
        let overall_score = u32::midpoint(structure_score, readability_score);

        DocumentQuality {
            overall_score,
            readability_score,
            structure_score,
            issues,
            suggestions,
        }
    }

    /// Calculate structure score
    fn calculate_structure_score(structure: &DocumentStructure, issues: &[QualityIssue]) -> u32 {
        let mut score = 100u32;

        // Deduct for issues
        for issue in issues {
            match issue.severity {
                IssueSeverity::Error => score = score.saturating_sub(15),
                IssueSeverity::Warning => score = score.saturating_sub(5),
                IssueSeverity::Info => score = score.saturating_sub(2),
            }
        }

        // Bonus for good structure
        if structure.title.is_some() {
            score = score.saturating_add(5).min(100);
        }
        if !structure.headings.is_empty() {
            score = score.saturating_add(5).min(100);
        }

        score
    }

    /// Calculate readability score (simplified Flesch-Kincaid style)
    fn calculate_readability_score(structure: &DocumentStructure) -> u32 {
        let words = structure.word_count;
        if words == 0 {
            return 50;
        }

        // Count sentences (rough estimate)
        let sentence_count = structure.plain_text.matches(['.', '!', '?']).count().max(1);

        // Average words per sentence
        let avg_words_per_sentence = words as f64 / sentence_count as f64;

        // Optimal is around 15-20 words per sentence
        let score = if avg_words_per_sentence < 10.0 {
            70 + ((avg_words_per_sentence / 10.0) * 20.0) as u32
        } else if avg_words_per_sentence <= 20.0 {
            90 + (10.0 - (avg_words_per_sentence - 15.0).abs()) as u32
        } else if avg_words_per_sentence <= 30.0 {
            70 - ((avg_words_per_sentence - 20.0) * 2.0) as u32
        } else {
            50
        };

        score.min(100)
    }
}

/// Document table of contents generator
pub struct TocGenerator;

impl TocGenerator {
    /// Generate table of contents from document structure
    #[must_use]
    pub fn generate(structure: &DocumentStructure) -> Vec<TocEntry> {
        structure
            .headings
            .iter()
            .map(|h| TocEntry {
                level: h.level,
                text: h.text.clone(),
                anchor: h.anchor.clone(),
            })
            .collect()
    }

    /// Generate table of contents as markdown
    #[must_use]
    pub fn generate_markdown(structure: &DocumentStructure) -> String {
        let mut result = String::new();

        for heading in &structure.headings {
            let indent = "  ".repeat((heading.level - 1) as usize);
            let anchor = heading
                .anchor
                .as_ref()
                .map(|a| format!("#{a}"))
                .unwrap_or_default();

            let _ = writeln!(result, "{}- [{}]({})", indent, heading.text, anchor);
        }

        result
    }
}

/// Table of contents entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// Heading level
    pub level: u8,
    /// Heading text
    pub text: String,
    /// Anchor link
    pub anchor: Option<String>,
}

/// Document metadata extractor
pub struct MetadataExtractor;

impl MetadataExtractor {
    /// Extract metadata from document
    #[must_use]
    pub fn extract(content: &str) -> DocumentMetadata {
        let structure = DocumentParser::parse(content);
        let quality = QualityAnalyzer::analyze(&structure);

        DocumentMetadata {
            format: structure.format,
            title: structure.title,
            word_count: structure.word_count,
            char_count: structure.char_count,
            reading_time_minutes: structure.reading_time_minutes,
            heading_count: structure.stats.heading_count,
            link_count: structure.stats.link_count,
            image_count: structure.stats.image_count,
            code_block_count: structure.stats.code_block_count,
            quality_score: quality.overall_score,
            external_links: structure.links.iter().filter(|l| l.is_external).count(),
            internal_links: structure.links.iter().filter(|l| !l.is_external).count(),
        }
    }
}

/// Document metadata summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Document format
    pub format: DocumentFormat,
    /// Document title
    pub title: Option<String>,
    /// Word count
    pub word_count: usize,
    /// Character count
    pub char_count: usize,
    /// Estimated reading time
    pub reading_time_minutes: u32,
    /// Number of headings
    pub heading_count: usize,
    /// Number of links
    pub link_count: usize,
    /// Number of images
    pub image_count: usize,
    /// Number of code blocks
    pub code_block_count: usize,
    /// Quality score
    pub quality_score: u32,
    /// External link count
    pub external_links: usize,
    /// Internal link count
    pub internal_links: usize,
}

/// Error type for PDF parsing
#[derive(Debug, Clone)]
pub enum PdfParseError {
    /// IO error reading PDF
    IoError(String),
    /// Invalid PDF format
    InvalidFormat(String),
    /// PDF parsing failed
    ParseError(String),
    /// Text extraction failed
    ExtractionError(String),
}

impl std::fmt::Display for PdfParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfParseError::IoError(e) => write!(f, "IO error: {e}"),
            PdfParseError::InvalidFormat(e) => write!(f, "Invalid PDF format: {e}"),
            PdfParseError::ParseError(e) => write!(f, "Parse error: {e}"),
            PdfParseError::ExtractionError(e) => write!(f, "Extraction error: {e}"),
        }
    }
}

impl std::error::Error for PdfParseError {}

/// PDF document parser
pub struct PdfParser;

impl PdfParser {
    /// Parse a PDF document from binary data
    pub fn parse(data: &[u8]) -> Result<DocumentStructure, PdfParseError> {
        use lopdf::Document;

        let doc = Document::load_mem(data).map_err(|e| PdfParseError::ParseError(e.to_string()))?;

        let mut all_text = String::new();
        let mut page_count = 0;

        // Extract text from all pages
        let pages = doc.get_pages();
        for (page_num, _) in &pages {
            page_count += 1;
            if let Ok(text) = Self::extract_page_text(&doc, *page_num) {
                all_text.push_str(&text);
                all_text.push('\n');
            }
        }

        let plain_text = Self::clean_extracted_text(&all_text);
        let word_count = plain_text.split_whitespace().count();
        let char_count = plain_text.chars().count();

        // Try to extract title from metadata or first heading
        let title = Self::extract_title(&doc, &plain_text);

        // Extract headings (based on text analysis)
        let headings = Self::detect_headings(&plain_text);
        let heading_count = headings.len();

        // Extract links from PDF
        let links = Self::extract_links(&doc);
        let link_count = links.len();

        Ok(DocumentStructure {
            format: DocumentFormat::Pdf,
            title,
            headings,
            links,
            images: Vec::new(), // PDF image extraction is complex
            code_blocks: Vec::new(),
            plain_text,
            word_count,
            char_count,
            reading_time_minutes: (word_count / 200).max(1) as u32,
            stats: DocumentStats {
                heading_count,
                paragraph_count: page_count,
                link_count,
                ..Default::default()
            },
        })
    }

    /// Extract text from a specific page
    fn extract_page_text(doc: &lopdf::Document, page_num: u32) -> Result<String, PdfParseError> {
        let page_id = doc
            .page_iter()
            .nth((page_num - 1) as usize)
            .ok_or_else(|| PdfParseError::ExtractionError(format!("Page {page_num} not found")))?;

        let content = doc
            .get_page_content(page_id)
            .map_err(|e| PdfParseError::ExtractionError(e.to_string()))?;

        // Parse content stream for text
        let text = Self::parse_content_stream(&content, doc);
        Ok(text)
    }

    /// Parse PDF content stream to extract text
    fn parse_content_stream(content: &[u8], doc: &lopdf::Document) -> String {
        use lopdf::content::Content;

        let mut text = String::new();

        if let Ok(content_obj) = Content::decode(content) {
            for operation in content_obj.operations {
                match operation.operator.as_str() {
                    "Tj" | "TJ" => {
                        // Text showing operators
                        for operand in &operation.operands {
                            Self::extract_text_from_object(operand, doc, &mut text);
                        }
                    }
                    "'" | "\"" => {
                        // Text with newline
                        text.push('\n');
                        for operand in &operation.operands {
                            Self::extract_text_from_object(operand, doc, &mut text);
                        }
                    }
                    _ => {}
                }
            }
        }

        text
    }

    /// Extract text from a PDF object
    fn extract_text_from_object(obj: &lopdf::Object, _doc: &lopdf::Document, text: &mut String) {
        use lopdf::Object;

        match obj {
            Object::String(bytes, _) => {
                // Try UTF-8 first, then PDFDocEncoding (Latin-1)
                if let Ok(s) = std::str::from_utf8(bytes) {
                    text.push_str(s);
                } else {
                    // Fall back to Latin-1
                    let s: String = bytes.iter().map(|&b| b as char).collect();
                    text.push_str(&s);
                }
            }
            Object::Array(arr) => {
                for item in arr {
                    match item {
                        Object::String(bytes, _) => {
                            if let Ok(s) = std::str::from_utf8(bytes) {
                                text.push_str(s);
                            } else {
                                let s: String = bytes.iter().map(|&b| b as char).collect();
                                text.push_str(&s);
                            }
                        }
                        Object::Integer(n) => {
                            // Negative numbers indicate kerning/spacing
                            if *n < -100 {
                                text.push(' ');
                            }
                        }
                        Object::Real(n) => {
                            if *n < -100.0 {
                                text.push(' ');
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Clean extracted text
    fn clean_extracted_text(text: &str) -> String {
        // Remove excessive whitespace and normalize
        let mut result = String::new();
        let mut last_was_space = true;
        let mut last_was_newline = true;

        for c in text.chars() {
            if c == '\n' || c == '\r' {
                if !last_was_newline {
                    result.push('\n');
                    last_was_newline = true;
                    last_was_space = true;
                }
            } else if c.is_whitespace() {
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            } else if c.is_control() {
                // Skip control characters
            } else {
                result.push(c);
                last_was_space = false;
                last_was_newline = false;
            }
        }

        result.trim().to_string()
    }

    /// Extract title from PDF metadata or content
    fn extract_title(doc: &lopdf::Document, text: &str) -> Option<String> {
        // Try to get title from PDF metadata
        if let Ok(info) = doc.trailer.get(b"Info") {
            if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(info.as_reference().ok()?) {
                if let Ok(lopdf::Object::String(bytes, _)) = dict.get(b"Title") {
                    if let Ok(s) = std::str::from_utf8(bytes) {
                        let title = s.trim();
                        if !title.is_empty() {
                            return Some(title.to_string());
                        }
                    }
                }
            }
        }

        // Fall back to first line that looks like a title
        for line in text.lines().take(10) {
            let trimmed = line.trim();
            if trimmed.len() > 3 && trimmed.len() < 200 {
                // Likely a title if it's reasonably sized and not a full paragraph
                let word_count = trimmed.split_whitespace().count();
                if word_count <= 15 && !trimmed.ends_with('.') {
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    /// Detect headings from text structure
    fn detect_headings(text: &str) -> Vec<Heading> {
        let mut headings = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let numbered_heading = regex::Regex::new(r"^(\d+\.)+\d*\s+[A-Z]").ok();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Skip empty or very long lines
            if trimmed.is_empty() || trimmed.len() > 200 {
                continue;
            }

            // Detect numbered headings (e.g., "1. Introduction", "1.2.3 Methods")
            if let Some(re) = &numbered_heading {
                if re.is_match(trimmed) {
                    let depth = trimmed.matches('.').count();
                    let level = (depth.min(5) + 1) as u8;
                    headings.push(Heading {
                        level,
                        text: trimmed.to_string(),
                        anchor: None,
                    });
                    continue;
                }
            }

            // Detect ALL CAPS headings (common in PDFs)
            let word_count = trimmed.split_whitespace().count();
            if (1..=10).contains(&word_count)
                && trimmed
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .all(char::is_uppercase)
                && trimmed.chars().any(char::is_alphabetic)
            {
                headings.push(Heading {
                    level: 2,
                    text: trimmed.to_string(),
                    anchor: None,
                });
                continue;
            }

            // Detect headings followed by blank line or significantly shorter
            if i + 1 < lines.len() {
                let next_line = lines[i + 1].trim();
                if next_line.is_empty() && word_count <= 8 && !trimmed.ends_with('.') {
                    // Check if it's capitalized like a title
                    if trimmed.chars().next().is_some_and(char::is_uppercase) {
                        headings.push(Heading {
                            level: 3,
                            text: trimmed.to_string(),
                            anchor: None,
                        });
                    }
                }
            }
        }

        headings
    }

    /// Extract links from PDF annotations
    fn extract_links(doc: &lopdf::Document) -> Vec<Link> {
        let mut links = Vec::new();

        for (_page_num, page_id) in doc.get_pages() {
            if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(page_id) {
                if let Ok(annots) = dict.get(b"Annots") {
                    Self::extract_links_from_annotations(doc, annots, &mut links);
                }
            }
        }

        links
    }

    /// Extract links from annotation array
    fn extract_links_from_annotations(
        doc: &lopdf::Document,
        annots: &lopdf::Object,
        links: &mut Vec<Link>,
    ) {
        let annot_refs = match annots {
            lopdf::Object::Array(arr) => arr.clone(),
            lopdf::Object::Reference(r) => {
                if let Ok(lopdf::Object::Array(arr)) = doc.get_object(*r) {
                    arr.clone()
                } else {
                    return;
                }
            }
            _ => return,
        };

        for annot_ref in annot_refs {
            let annot = match &annot_ref {
                lopdf::Object::Reference(r) => doc.get_object(*r).ok().cloned(),
                obj => Some(obj.clone()),
            };

            if let Some(lopdf::Object::Dictionary(dict)) = annot {
                // Check if it's a link annotation
                if let Ok(lopdf::Object::Name(subtype)) = dict.get(b"Subtype") {
                    if subtype == b"Link" {
                        // Extract URL from action
                        if let Ok(action) = dict.get(b"A") {
                            Self::extract_url_from_action(doc, action, links);
                        }
                    }
                }
            }
        }
    }

    /// Extract URL from PDF action
    fn extract_url_from_action(
        doc: &lopdf::Document,
        action: &lopdf::Object,
        links: &mut Vec<Link>,
    ) {
        let action_dict = match action {
            lopdf::Object::Dictionary(dict) => dict.clone(),
            lopdf::Object::Reference(r) => {
                if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(*r) {
                    dict.clone()
                } else {
                    return;
                }
            }
            _ => return,
        };

        // Check for URI action
        if let Ok(lopdf::Object::Name(s)) = action_dict.get(b"S") {
            if s == b"URI" {
                if let Ok(lopdf::Object::String(bytes, _)) = action_dict.get(b"URI") {
                    if let Ok(url) = std::str::from_utf8(bytes) {
                        let is_external = url.starts_with("http://")
                            || url.starts_with("https://")
                            || url.starts_with("mailto:");
                        links.push(Link {
                            url: url.to_string(),
                            text: String::new(), // PDF links often don't have separate text
                            title: None,
                            is_external,
                        });
                    }
                }
            }
        }
    }
}

/// PDF document metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMetadata {
    /// PDF version
    pub version: String,
    /// Page count
    pub page_count: usize,
    /// Document title
    pub title: Option<String>,
    /// Document author
    pub author: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Document keywords
    pub keywords: Option<String>,
    /// Creator application
    pub creator: Option<String>,
    /// Producer application
    pub producer: Option<String>,
    /// Creation date
    pub creation_date: Option<String>,
    /// Modification date
    pub modification_date: Option<String>,
    /// Whether the PDF is encrypted
    pub is_encrypted: bool,
}

impl PdfParser {
    /// Extract metadata from PDF
    pub fn extract_metadata(data: &[u8]) -> Result<PdfMetadata, PdfParseError> {
        use lopdf::Document;

        let doc = Document::load_mem(data).map_err(|e| PdfParseError::ParseError(e.to_string()))?;

        let page_count = doc.get_pages().len();
        let version = doc.version.clone();
        let is_encrypted = doc.is_encrypted();

        let mut metadata = PdfMetadata {
            version,
            page_count,
            title: None,
            author: None,
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            modification_date: None,
            is_encrypted,
        };

        // Extract info dictionary
        if let Ok(info_ref) = doc.trailer.get(b"Info") {
            if let Ok(r) = info_ref.as_reference() {
                if let Ok(lopdf::Object::Dictionary(dict)) = doc.get_object(r) {
                    metadata.title = Self::get_string_from_dict(dict, b"Title");
                    metadata.author = Self::get_string_from_dict(dict, b"Author");
                    metadata.subject = Self::get_string_from_dict(dict, b"Subject");
                    metadata.keywords = Self::get_string_from_dict(dict, b"Keywords");
                    metadata.creator = Self::get_string_from_dict(dict, b"Creator");
                    metadata.producer = Self::get_string_from_dict(dict, b"Producer");
                    metadata.creation_date = Self::get_string_from_dict(dict, b"CreationDate");
                    metadata.modification_date = Self::get_string_from_dict(dict, b"ModDate");
                }
            }
        }

        Ok(metadata)
    }

    /// Get string value from dictionary
    fn get_string_from_dict(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
        if let Ok(lopdf::Object::String(bytes, _)) = dict.get(key) {
            if let Ok(s) = std::str::from_utf8(bytes) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection_markdown() {
        let content = "# Hello World\n\nThis is a **test** document.";
        assert_eq!(DocumentFormat::detect(content), DocumentFormat::Markdown);
    }

    #[test]
    fn test_format_detection_html() {
        let content = "<!DOCTYPE html><html><body><p>Hello</p></body></html>";
        assert_eq!(DocumentFormat::detect(content), DocumentFormat::Html);
    }

    #[test]
    fn test_markdown_heading_parsing() {
        let content = "# Title\n\n## Section 1\n\n### Subsection\n\nSome text.";
        let structure = DocumentParser::parse(content);

        assert_eq!(structure.headings.len(), 3);
        assert_eq!(structure.headings[0].level, 1);
        assert_eq!(structure.headings[0].text, "Title");
        assert_eq!(structure.headings[1].level, 2);
        assert_eq!(structure.headings[2].level, 3);
    }

    #[test]
    fn test_markdown_link_extraction() {
        let content = "Check out [Rust](https://rust-lang.org) and [this](./local.md).";
        let structure = DocumentParser::parse(content);

        assert_eq!(structure.links.len(), 2);
        assert!(structure.links[0].is_external);
        assert!(!structure.links[1].is_external);
    }

    #[test]
    fn test_markdown_image_extraction() {
        let content = "![Alt text](image.png \"Title\")";
        let structure = DocumentParser::parse(content);

        assert_eq!(structure.images.len(), 1);
        assert_eq!(structure.images[0].alt, "Alt text");
        assert_eq!(structure.images[0].src, "image.png");
    }

    #[test]
    fn test_markdown_code_block_extraction() {
        let content = "```rust\nfn main() {}\n```";
        let structure = DocumentParser::parse(content);

        assert_eq!(structure.code_blocks.len(), 1);
        assert_eq!(structure.code_blocks[0].language, Some("rust".to_string()));
    }

    #[test]
    fn test_html_to_plain_text() {
        let html = "<p>Hello <strong>world</strong>!</p>";
        let plain = DocumentParser::html_to_plain_text(html);
        assert_eq!(plain, "Hello world !");
    }

    #[test]
    fn test_quality_analysis() {
        let content = "# My Document\n\nThis is a test document with some content.\n\n## Section\n\nMore content here.";
        let structure = DocumentParser::parse(content);
        let quality = QualityAnalyzer::analyze(&structure);

        assert!(quality.overall_score > 70);
        assert!(
            quality.issues.is_empty()
                || quality
                    .issues
                    .iter()
                    .all(|i| i.severity != IssueSeverity::Error)
        );
    }
}
