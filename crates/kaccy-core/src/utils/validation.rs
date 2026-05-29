//! Validation utilities and helpers

use regex::Regex;
use rust_decimal::Decimal;
use std::sync::OnceLock;

/// Email validation regex
static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();

/// Username validation regex (alphanumeric, underscore, hyphen, 3-30 chars)
static USERNAME_REGEX: OnceLock<Regex> = OnceLock::new();

/// Token symbol validation regex (alphanumeric, starting with $, 2-10 chars)
static SYMBOL_REGEX: OnceLock<Regex> = OnceLock::new();

/// Bitcoin address validation regex (simplified)
static BTC_ADDRESS_REGEX: OnceLock<Regex> = OnceLock::new();

/// Validation result type
pub type ValidationResult = Result<(), ValidationError>;

/// Validation error
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct ValidationError(pub String);

/// Validator trait for custom validation logic
pub trait Validator {
    /// Run validation and return the result.
    fn validate(&self) -> ValidationResult;
}

/// Email validator
pub struct EmailValidator;

impl EmailValidator {
    /// Validate email format
    pub fn validate(email: &str) -> ValidationResult {
        let regex = EMAIL_REGEX.get_or_init(|| {
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
                .expect("email regex pattern is valid")
        });

        if email.is_empty() {
            return Err(ValidationError("Email is required".to_string()));
        }

        if email.len() > 255 {
            return Err(ValidationError("Email is too long".to_string()));
        }

        if !regex.is_match(email) {
            return Err(ValidationError("Invalid email format".to_string()));
        }

        Ok(())
    }
}

/// Username validator
pub struct UsernameValidator;

impl UsernameValidator {
    /// Validate username format
    pub fn validate(username: &str) -> ValidationResult {
        let regex = USERNAME_REGEX.get_or_init(|| {
            Regex::new(r"^[a-zA-Z0-9_-]{3,30}$").expect("username regex pattern is valid")
        });

        if username.is_empty() {
            return Err(ValidationError("Username is required".to_string()));
        }

        if !regex.is_match(username) {
            return Err(ValidationError(
                "Username must be 3-30 characters, alphanumeric, underscore or hyphen only"
                    .to_string(),
            ));
        }

        // Reserved usernames
        let reserved = &["admin", "system", "root", "kaccy", "api"];
        if reserved.contains(&username.to_lowercase().as_str()) {
            return Err(ValidationError("Username is reserved".to_string()));
        }

        Ok(())
    }
}

/// Token symbol validator
pub struct SymbolValidator;

impl SymbolValidator {
    /// Validate token symbol format
    pub fn validate(symbol: &str) -> ValidationResult {
        let regex = SYMBOL_REGEX.get_or_init(|| {
            Regex::new(r"^\$[A-Z0-9]{1,9}$").expect("symbol regex pattern is valid")
        });

        if symbol.is_empty() {
            return Err(ValidationError("Symbol is required".to_string()));
        }

        if !regex.is_match(symbol) {
            return Err(ValidationError(
                "Symbol must start with $ and contain 1-9 uppercase alphanumeric characters"
                    .to_string(),
            ));
        }

        // Reserved symbols
        let reserved = &["$BTC", "$ETH", "$USD", "$KACCY"];
        if reserved.contains(&symbol) {
            return Err(ValidationError("Symbol is reserved".to_string()));
        }

        Ok(())
    }
}

/// Bitcoin address validator
pub struct BtcAddressValidator;

impl BtcAddressValidator {
    /// Validate Bitcoin address format (basic validation)
    pub fn validate(address: &str) -> ValidationResult {
        // Basic length and format check for Bitcoin addresses
        // bc1: bech32 (26-90 chars)
        // 1: P2PKH (26-35 chars)
        // 3: P2SH (26-35 chars)
        let regex = BTC_ADDRESS_REGEX.get_or_init(|| {
            Regex::new(r"^(bc1[a-z0-9]{39,87}|[13][a-km-zA-HJ-NP-Z1-9]{25,34})$")
                .expect("bitcoin address regex pattern is valid")
        });

        if address.is_empty() {
            return Err(ValidationError("Bitcoin address is required".to_string()));
        }

        if address.len() < 26 || address.len() > 90 {
            return Err(ValidationError(
                "Bitcoin address length invalid".to_string(),
            ));
        }

        if !regex.is_match(address) {
            return Err(ValidationError(
                "Invalid Bitcoin address format".to_string(),
            ));
        }

        Ok(())
    }
}

/// String length validator
pub struct StringLengthValidator {
    min: usize,
    max: usize,
}

impl StringLengthValidator {
    /// Create a new string length validator with minimum and maximum length bounds.
    pub fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }

    /// Validate that `value` satisfies the configured length constraints.
    pub fn validate(&self, value: &str, field_name: &str) -> ValidationResult {
        let len = value.len();

        if len < self.min {
            return Err(ValidationError(format!(
                "{} must be at least {} characters",
                field_name, self.min
            )));
        }

        if len > self.max {
            return Err(ValidationError(format!(
                "{} must be at most {} characters",
                field_name, self.max
            )));
        }

        Ok(())
    }
}

/// Numeric range validator
pub struct NumericRangeValidator {
    min: Decimal,
    max: Decimal,
}

impl NumericRangeValidator {
    /// Create a new numeric range validator with inclusive min and max bounds.
    pub fn new(min: Decimal, max: Decimal) -> Self {
        Self { min, max }
    }

    /// Validate that `value` falls within the configured numeric range.
    pub fn validate(&self, value: Decimal, field_name: &str) -> ValidationResult {
        if value < self.min {
            return Err(ValidationError(format!(
                "{} must be at least {}",
                field_name, self.min
            )));
        }

        if value > self.max {
            return Err(ValidationError(format!(
                "{} must be at most {}",
                field_name, self.max
            )));
        }

        Ok(())
    }

    /// Validate that `value` is strictly positive and within the configured range.
    pub fn validate_positive(&self, value: Decimal, field_name: &str) -> ValidationResult {
        if value <= Decimal::ZERO {
            return Err(ValidationError(format!("{} must be positive", field_name)));
        }

        self.validate(value, field_name)
    }
}

/// URL validator
pub struct UrlValidator;

impl UrlValidator {
    /// Validate URL format
    pub fn validate(url: &str) -> ValidationResult {
        if url.is_empty() {
            return Err(ValidationError("URL is required".to_string()));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ValidationError(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        if url.len() > 2048 {
            return Err(ValidationError("URL is too long".to_string()));
        }

        Ok(())
    }

    /// Validate HTTPS URL
    pub fn validate_https(url: &str) -> ValidationResult {
        Self::validate(url)?;

        if !url.starts_with("https://") {
            return Err(ValidationError("URL must use HTTPS".to_string()));
        }

        Ok(())
    }
}

/// Collection validator
pub struct CollectionValidator;

impl CollectionValidator {
    /// Validate collection is not empty
    pub fn not_empty<T>(collection: &[T], field_name: &str) -> ValidationResult {
        if collection.is_empty() {
            return Err(ValidationError(format!("{} cannot be empty", field_name)));
        }
        Ok(())
    }

    /// Validate collection size
    pub fn size<T>(collection: &[T], min: usize, max: usize, field_name: &str) -> ValidationResult {
        let len = collection.len();

        if len < min {
            return Err(ValidationError(format!(
                "{} must contain at least {} items",
                field_name, min
            )));
        }

        if len > max {
            return Err(ValidationError(format!(
                "{} must contain at most {} items",
                field_name, max
            )));
        }

        Ok(())
    }
}

/// Aggregate multiple validation results
pub fn validate_all(validations: Vec<ValidationResult>) -> ValidationResult {
    let errors: Vec<String> = validations
        .into_iter()
        .filter_map(|r| r.err())
        .map(|e| e.0)
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ValidationError(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_email_validation() {
        assert!(EmailValidator::validate("user@example.com").is_ok());
        assert!(EmailValidator::validate("test.user+tag@example.co.uk").is_ok());
        assert!(EmailValidator::validate("invalid@").is_err());
        assert!(EmailValidator::validate("@example.com").is_err());
        assert!(EmailValidator::validate("not-an-email").is_err());
        assert!(EmailValidator::validate("").is_err());
    }

    #[test]
    fn test_username_validation() {
        assert!(UsernameValidator::validate("validuser").is_ok());
        assert!(UsernameValidator::validate("user_123").is_ok());
        assert!(UsernameValidator::validate("user-name").is_ok());
        assert!(UsernameValidator::validate("ab").is_err()); // Too short
        assert!(UsernameValidator::validate("user@name").is_err()); // Invalid char
        assert!(UsernameValidator::validate("admin").is_err()); // Reserved
        assert!(UsernameValidator::validate("").is_err());
    }

    #[test]
    fn test_symbol_validation() {
        assert!(SymbolValidator::validate("$TEST").is_ok());
        assert!(SymbolValidator::validate("$ABC123").is_ok());
        assert!(SymbolValidator::validate("TEST").is_err()); // No $
        assert!(SymbolValidator::validate("$test").is_err()); // Lowercase
        assert!(SymbolValidator::validate("$TOOLONGNAME").is_err()); // Too long
        assert!(SymbolValidator::validate("$BTC").is_err()); // Reserved
        assert!(SymbolValidator::validate("").is_err());
    }

    #[test]
    fn test_btc_address_validation() {
        // Valid P2PKH address (starts with 1)
        assert!(BtcAddressValidator::validate("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").is_ok());
        // Valid P2SH address (starts with 3)
        assert!(BtcAddressValidator::validate("3J98t1WpEZ73CNmYviecrnyiWrnqRhWNLy").is_ok());
        // Valid Bech32 address (starts with bc1)
        assert!(
            BtcAddressValidator::validate("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq").is_ok()
        );
        // Invalid addresses
        assert!(BtcAddressValidator::validate("invalid").is_err());
        assert!(BtcAddressValidator::validate("").is_err());
        assert!(BtcAddressValidator::validate("bc1short").is_err());
    }

    #[test]
    fn test_string_length_validation() {
        let validator = StringLengthValidator::new(3, 10);
        assert!(validator.validate("test", "field").is_ok());
        assert!(validator.validate("ab", "field").is_err());
        assert!(validator.validate("toolongstring", "field").is_err());
    }

    #[test]
    fn test_numeric_range_validation() {
        let validator = NumericRangeValidator::new(dec!(0), dec!(100));
        assert!(validator.validate(dec!(50), "field").is_ok());
        assert!(validator.validate(dec!(-1), "field").is_err());
        assert!(validator.validate(dec!(101), "field").is_err());

        assert!(validator.validate_positive(dec!(10), "field").is_ok());
        assert!(validator.validate_positive(dec!(0), "field").is_err());
        assert!(validator.validate_positive(dec!(-5), "field").is_err());
    }

    #[test]
    fn test_url_validation() {
        assert!(UrlValidator::validate("https://example.com").is_ok());
        assert!(UrlValidator::validate("http://example.com/path").is_ok());
        assert!(UrlValidator::validate("example.com").is_err());
        assert!(UrlValidator::validate("ftp://example.com").is_err());
        assert!(UrlValidator::validate("").is_err());

        assert!(UrlValidator::validate_https("https://example.com").is_ok());
        assert!(UrlValidator::validate_https("http://example.com").is_err());
    }

    #[test]
    fn test_collection_validation() {
        let items = vec![1, 2, 3];
        assert!(CollectionValidator::not_empty(&items, "items").is_ok());
        assert!(CollectionValidator::size(&items, 1, 5, "items").is_ok());
        assert!(CollectionValidator::size(&items, 5, 10, "items").is_err());

        let empty: Vec<i32> = vec![];
        assert!(CollectionValidator::not_empty(&empty, "items").is_err());
    }

    #[test]
    fn test_validate_all() {
        let all_ok = vec![Ok(()), Ok(()), Ok(())];
        assert!(validate_all(all_ok).is_ok());

        let some_err = vec![
            Ok(()),
            Err(ValidationError("Error 1".to_string())),
            Err(ValidationError("Error 2".to_string())),
        ];
        let result = validate_all(some_err);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().0;
        assert!(error_msg.contains("Error 1"));
        assert!(error_msg.contains("Error 2"));
    }
}
