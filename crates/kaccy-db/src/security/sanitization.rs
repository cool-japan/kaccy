extern crate alloc;

use regex::Regex;
use std::sync::LazyLock;

/// Sanitizer for SQL query parameters to prevent SQL injection
#[derive(Debug, Clone)]
pub struct ParameterSanitizer {
    /// Maximum allowed parameter length
    max_length: usize,
    /// Patterns to detect potential SQL injection
    dangerous_patterns: Vec<Regex>,
    /// Whether to allow special characters
    allow_special_chars: bool,
}

// Use LazyLock to initialize regex patterns once
static SQL_INJECTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)(union\s+select|insert\s+into|drop\s+table|delete\s+from)").unwrap(),
        Regex::new(r"(?i)(exec\s*\(|execute\s*\()").unwrap(),
        Regex::new(r"(?i)(xp_cmdshell|sp_executesql)").unwrap(),
        Regex::new(r"(--|/\*|\*/|;)").unwrap(),
        Regex::new(r#"['\";]{2,}"#).unwrap(),
    ]
});

impl Default for ParameterSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterSanitizer {
    /// Create a new parameter sanitizer with default settings
    pub fn new() -> Self {
        Self {
            max_length: 1000,
            dangerous_patterns: SQL_INJECTION_PATTERNS.clone(),
            allow_special_chars: true,
        }
    }

    /// Create a strict sanitizer that blocks all special characters
    pub fn strict() -> Self {
        Self {
            max_length: 500,
            dangerous_patterns: SQL_INJECTION_PATTERNS.clone(),
            allow_special_chars: false,
        }
    }

    /// Set the maximum allowed parameter length
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Sanitize a string parameter
    pub fn sanitize(&self, input: &str) -> Result<String, SanitizationError> {
        if input.len() > self.max_length {
            return Err(SanitizationError::TooLong {
                actual: input.len(),
                max: self.max_length,
            });
        }

        for pattern in &self.dangerous_patterns {
            if pattern.is_match(input) {
                return Err(SanitizationError::DangerousPattern {
                    pattern: pattern.as_str().to_string(),
                });
            }
        }

        if !self.allow_special_chars
            && input
                .chars()
                .any(|c| !c.is_alphanumeric() && c != ' ' && c != '_' && c != '-')
        {
            return Err(SanitizationError::SpecialCharactersNotAllowed);
        }

        Ok(input.to_string())
    }

    /// Sanitize an email address
    pub fn sanitize_email(&self, email: &str) -> Result<String, SanitizationError> {
        if !email.contains('@') || !email.contains('.') {
            return Err(SanitizationError::InvalidFormat("email".to_string()));
        }
        Ok(email.to_string())
    }

    /// Sanitize a username (alphanumeric with underscores and hyphens)
    pub fn sanitize_username(&self, username: &str) -> Result<String, SanitizationError> {
        if username.len() < 3 || username.len() > 50 {
            return Err(SanitizationError::InvalidLength { min: 3, max: 50 });
        }

        if !username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SanitizationError::InvalidFormat("username".to_string()));
        }

        Ok(username.to_string())
    }

    /// Validate and sanitize a UUID string
    pub fn sanitize_uuid(&self, uuid: &str) -> Result<String, SanitizationError> {
        if uuid.len() != 36 || uuid.chars().filter(|&c| c == '-').count() != 4 {
            return Err(SanitizationError::InvalidFormat("uuid".to_string()));
        }
        Ok(uuid.to_lowercase())
    }
}

/// PII (Personally Identifiable Information) redactor for logs
#[derive(Debug, Clone)]
pub struct PiiRedactor {
    /// Replacement text
    replacement: String,
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiRedactor {
    /// Create a new PII redactor
    pub fn new() -> Self {
        Self {
            replacement: "[REDACTED]".to_string(),
        }
    }

    /// Redact PII from a string (simple implementation)
    pub fn redact(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Redact email-like patterns
        if input.contains('@') {
            let words: Vec<&str> = input.split_whitespace().collect();
            for word in words {
                if word.contains('@') && word.contains('.') {
                    let colon = String::from(":");
                    let email_label = String::from("EMAIL");
                    let redacted = self.replacement.clone() + &colon + &email_label;
                    result = result.replace(word, &redacted);
                }
            }
        }

        result
    }

    /// Redact specific fields from a JSON-like structure
    pub fn redact_fields(&self, input: &str, fields: &[&str]) -> String {
        let mut result = input.to_string();

        for field in fields {
            // Simple word-based replacement
            if let Some(pos) = result.find(field) {
                // Check if there's a colon after the field name
                let search_start = pos + field.len();
                if let Some(rest) = result.get(search_start..) {
                    if let Some(colon_pos) = rest.find(':') {
                        let value_start = search_start + colon_pos + 1;
                        // Find the end of the value (comma or closing brace)
                        if let Some(value_rest) = result.get(value_start..) {
                            let value_end_offset = value_rest
                                .chars()
                                .position(|c| c == ',' || c == '}')
                                .unwrap_or(value_rest.len());

                            let before = &result[..value_start];
                            let after = &result[value_start + value_end_offset..];
                            result = String::from(before) + &self.replacement + after;
                        }
                    }
                }
            }
        }

        result
    }
}

/// Sensitive data masker for displaying partial information
#[derive(Debug, Clone)]
pub struct DataMasker {
    /// Character to use for masking
    mask_char: char,
}

impl Default for DataMasker {
    fn default() -> Self {
        Self::new()
    }
}

impl DataMasker {
    /// Create a new data masker
    pub fn new() -> Self {
        Self { mask_char: '*' }
    }

    /// Mask all but the last N characters
    pub fn mask_except_last(&self, input: &str, visible: usize) -> String {
        let len = input.chars().count();
        if len <= visible {
            return input.to_string();
        }

        let mask_count = len - visible;
        let masked: String = std::iter::repeat_n(self.mask_char, mask_count).collect();
        let visible_part: String = input.chars().skip(mask_count).collect();

        format!("{}{}", masked, visible_part)
    }

    /// Mask all but the first N characters
    pub fn mask_except_first(&self, input: &str, visible: usize) -> String {
        let len = input.chars().count();
        if len <= visible {
            return input.to_string();
        }

        let visible_part: String = input.chars().take(visible).collect();
        let mask_count = len - visible;
        let masked: String = std::iter::repeat_n(self.mask_char, mask_count).collect();

        format!("{}{}", visible_part, masked)
    }

    /// Mask the middle portion of a string
    pub fn mask_middle(&self, input: &str, visible_start: usize, visible_end: usize) -> String {
        let len = input.chars().count();
        if len <= visible_start + visible_end {
            return input.to_string();
        }

        let start: String = input.chars().take(visible_start).collect();
        let end: String = input.chars().skip(len - visible_end).collect();
        let mask_count = len - visible_start - visible_end;
        let masked: String = std::iter::repeat_n(self.mask_char, mask_count).collect();

        format!("{}{}{}", start, masked, end)
    }

    /// Mask an email address (keep first char and domain)
    pub fn mask_email(&self, email: &str) -> String {
        if let Some(at_pos) = email.find('@') {
            let (local, domain) = email.split_at(at_pos);
            if local.len() > 1 {
                let first_char = local.chars().next().unwrap();
                let mask_count = local.len() - 1;
                let masked: String = std::iter::repeat_n(self.mask_char, mask_count).collect();
                format!("{}{}{}", first_char, masked, domain)
            } else {
                email.to_string()
            }
        } else {
            email.to_string()
        }
    }

    /// Mask a credit card number (show last 4 digits)
    pub fn mask_credit_card(&self, cc: &str) -> String {
        self.mask_except_last(cc, 4)
    }
}

/// Errors that can occur during parameter sanitization
#[derive(Debug, Clone)]
pub enum SanitizationError {
    /// Input exceeds the maximum allowed length.
    TooLong {
        /// Actual length of the input.
        actual: usize,
        /// Maximum allowed length.
        max: usize,
    },
    /// Input matched a dangerous SQL injection pattern.
    DangerousPattern {
        /// The regex pattern that was matched.
        pattern: String,
    },
    /// Input contains special characters that are not permitted.
    SpecialCharactersNotAllowed,
    /// Input does not conform to the expected format (e.g., email, UUID).
    InvalidFormat(String),
    /// Input length is outside the allowed range.
    InvalidLength {
        /// Minimum required length.
        min: usize,
        /// Maximum allowed length.
        max: usize,
    },
}

impl std::fmt::Display for SanitizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            SanitizationError::TooLong { actual, max } => {
                alloc::format!("Parameter too long: {} characters (max {})", actual, max)
            }
            SanitizationError::DangerousPattern { pattern } => {
                alloc::format!("Dangerous pattern: {}", pattern)
            }
            SanitizationError::SpecialCharactersNotAllowed => String::from("Invalid characters"),
            SanitizationError::InvalidFormat(format_type) => {
                alloc::format!("Invalid format: {}", format_type)
            }
            SanitizationError::InvalidLength { min, max } => {
                alloc::format!("Length must be {} to {}", min, max)
            }
        };
        write!(f, "{}", msg)
    }
}

impl std::error::Error for SanitizationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_sanitizer_basic() {
        let sanitizer = ParameterSanitizer::new();
        assert!(sanitizer.sanitize("normal_text").is_ok());
    }

    #[test]
    fn test_parameter_sanitizer_sql_injection() {
        let sanitizer = ParameterSanitizer::new();
        let mut malicious_input = String::from("'");
        malicious_input.push_str("; DROP TABLE users; --");
        assert!(sanitizer.sanitize(&malicious_input).is_err());
    }

    #[test]
    fn test_parameter_sanitizer_too_long() {
        let sanitizer = ParameterSanitizer::new().with_max_length(10);
        assert!(sanitizer.sanitize("this is a very long string").is_err());
    }

    #[test]
    fn test_parameter_sanitizer_email() {
        let sanitizer = ParameterSanitizer::new();
        assert!(sanitizer.sanitize_email("user@example.com").is_ok());
        assert!(sanitizer.sanitize_email("invalid").is_err());
    }

    #[test]
    fn test_parameter_sanitizer_username() {
        let sanitizer = ParameterSanitizer::new();
        assert!(sanitizer.sanitize_username("valid_user123").is_ok());
        assert!(sanitizer.sanitize_username("ab").is_err()); // too short
        assert!(sanitizer.sanitize_username("user@invalid").is_err()); // invalid chars
    }

    #[test]
    fn test_parameter_sanitizer_uuid() {
        let sanitizer = ParameterSanitizer::new();
        assert!(sanitizer
            .sanitize_uuid("550e8400-e29b-41d4-a716-446655440000")
            .is_ok());
        assert!(sanitizer.sanitize_uuid("not-a-uuid").is_err());
    }

    #[test]
    fn test_pii_redactor() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact("Contact user@example.com for details");
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("user@example.com"));
    }

    #[test]
    fn test_pii_redactor_fields() {
        let redactor = PiiRedactor::new();
        let input = "email: user@test.com, name: John";
        let result = redactor.redact_fields(input, &["email"]);
        assert!(result.contains("[REDACTED]"));
    }

    #[test]
    fn test_data_masker_last() {
        let masker = DataMasker::new();
        assert_eq!(masker.mask_except_last("1234567890", 4), "******7890");
    }

    #[test]
    fn test_data_masker_first() {
        let masker = DataMasker::new();
        assert_eq!(masker.mask_except_first("1234567890", 4), "1234******");
    }

    #[test]
    fn test_data_masker_middle() {
        let masker = DataMasker::new();
        assert_eq!(masker.mask_middle("1234567890", 2, 2), "12******90");
    }

    #[test]
    fn test_data_masker_email() {
        let masker = DataMasker::new();
        let masked = masker.mask_email("user@example.com");
        assert!(masked.starts_with('u'));
        assert!(masked.contains("@example.com"));
    }

    #[test]
    fn test_data_masker_credit_card() {
        let masker = DataMasker::new();
        assert_eq!(
            masker.mask_credit_card("1234567890123456"),
            "************3456"
        );
    }
}
