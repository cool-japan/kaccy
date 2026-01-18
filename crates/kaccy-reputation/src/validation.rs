//! # Input Validation Utilities
//!
//! Provides comprehensive validation functions for all user-facing inputs,
//! business constraints, and data integrity rules.
//!
//! ## Overview
//!
//! All validation functions return `ValidationResult<()>`, which is a type alias
//! for `Result<(), ReputationError>`. This allows for consistent error handling
//! across the crate.
//!
//! ## Usage Examples
//!
//! ### Validating User Input
//!
//! ```rust
//! use kaccy_reputation::*;
//! use rust_decimal_macros::dec;
//!
//! # fn example() -> Result<(), ReputationError> {
//! // Validate commitment title
//! validate_title("Complete project deliverable")?;
//!
//! // Validate score value
//! validate_score(dec!(750))?;
//!
//! // Validate rating (1-5 stars)
//! validate_rating(4)?;
//!
//! // Validate deadline (must be 24h-12mo in future)
//! let deadline = chrono::Utc::now() + chrono::Duration::days(7);
//! validate_deadline(deadline)?;
//!
//! println!("All validations passed!");
//! # Ok(())
//! # }
//! ```
//!
//! ### Handling Validation Errors
//!
//! ```rust
//! use kaccy_reputation::*;
//!
//! # fn example() {
//! match validate_title("ab") {  // Too short (min 3 chars)
//!     Ok(_) => println!("Valid"),
//!     Err(e) => println!("Validation failed: {}", e),
//! }
//! # }
//! ```
//!
//! ## Validation Constants
//!
//! - `MIN_TITLE_LENGTH`: 3 characters
//! - `MAX_TITLE_LENGTH`: 200 characters
//! - `MAX_DESCRIPTION_LENGTH`: 2000 characters
//! - `MAX_FEEDBACK_LENGTH`: 1000 characters
//! - `MIN_DEADLINE_HOURS`: 24 hours (prevents last-minute commitments)
//! - `MAX_DEADLINE_MONTHS`: 12 months (prevents unrealistic far-future commitments)
//!
//! ## Best Practices
//!
//! - **Validate early**: Call validation functions before database operations
//! - **Use constants**: Reference validation constants in UI to match backend
//! - **Provide feedback**: Show specific validation errors to users
//! - **Chain validations**: Use `?` operator to fail fast on first error

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::ReputationError;

/// Minimum commitment title length
pub const MIN_TITLE_LENGTH: usize = 3;

/// Maximum commitment title length
pub const MAX_TITLE_LENGTH: usize = 200;

/// Maximum commitment description length
pub const MAX_DESCRIPTION_LENGTH: usize = 2000;

/// Maximum feedback length for ratings
pub const MAX_FEEDBACK_LENGTH: usize = 1000;

/// Minimum deadline hours in the future
pub const MIN_DEADLINE_HOURS: i64 = 24;

/// Maximum deadline months in the future
pub const MAX_DEADLINE_MONTHS: i64 = 12;

/// Validation result type
pub type ValidationResult<T> = Result<T, ReputationError>;

/// Validate a commitment title
pub fn validate_title(title: &str) -> ValidationResult<()> {
    let trimmed = title.trim();

    if trimmed.is_empty() {
        return Err(ReputationError::Validation(
            "Title cannot be empty".to_string(),
        ));
    }

    if trimmed.len() < MIN_TITLE_LENGTH {
        return Err(ReputationError::Validation(format!(
            "Title must be at least {} characters, got {}",
            MIN_TITLE_LENGTH,
            trimmed.len()
        )));
    }

    if trimmed.len() > MAX_TITLE_LENGTH {
        return Err(ReputationError::Validation(format!(
            "Title must be at most {} characters, got {}",
            MAX_TITLE_LENGTH,
            trimmed.len()
        )));
    }

    Ok(())
}

/// Validate a commitment description
pub fn validate_description(description: &str) -> ValidationResult<()> {
    let trimmed = description.trim();

    if trimmed.is_empty() {
        return Err(ReputationError::Validation(
            "Description cannot be empty".to_string(),
        ));
    }

    if trimmed.len() > MAX_DESCRIPTION_LENGTH {
        return Err(ReputationError::Validation(format!(
            "Description must be at most {} characters, got {}",
            MAX_DESCRIPTION_LENGTH,
            trimmed.len()
        )));
    }

    Ok(())
}

/// Validate a deadline is within acceptable range
pub fn validate_deadline(deadline: DateTime<Utc>) -> ValidationResult<()> {
    let now = Utc::now();

    // Check if deadline is in the past
    if deadline <= now {
        return Err(ReputationError::Validation(
            "Deadline must be in the future".to_string(),
        ));
    }

    // Check minimum deadline
    let min_deadline = now + chrono::Duration::hours(MIN_DEADLINE_HOURS);
    if deadline < min_deadline {
        return Err(ReputationError::Validation(format!(
            "Deadline must be at least {} hours in the future",
            MIN_DEADLINE_HOURS
        )));
    }

    // Check maximum deadline
    let max_deadline = now + chrono::Duration::days(MAX_DEADLINE_MONTHS * 30);
    if deadline > max_deadline {
        return Err(ReputationError::Validation(format!(
            "Deadline must be at most {} months in the future",
            MAX_DEADLINE_MONTHS
        )));
    }

    Ok(())
}

/// Validate rating value (1-5)
pub fn validate_rating(rating: i32) -> ValidationResult<()> {
    if !(1..=5).contains(&rating) {
        return Err(ReputationError::InvalidRating(format!(
            "Rating must be between 1 and 5, got {}",
            rating
        )));
    }
    Ok(())
}

/// Validate feedback text
pub fn validate_feedback(feedback: &str) -> ValidationResult<()> {
    if feedback.len() > MAX_FEEDBACK_LENGTH {
        return Err(ReputationError::Validation(format!(
            "Feedback must be at most {} characters, got {}",
            MAX_FEEDBACK_LENGTH,
            feedback.len()
        )));
    }
    Ok(())
}

/// Validate a score value is within acceptable range
pub fn validate_score(score: Decimal) -> ValidationResult<()> {
    if score < Decimal::ZERO {
        return Err(ReputationError::Validation(
            "Score cannot be negative".to_string(),
        ));
    }

    if score > Decimal::from(1000) {
        return Err(ReputationError::Validation(
            "Score cannot exceed 1000".to_string(),
        ));
    }

    Ok(())
}

/// Validate a UUID is not nil
pub fn validate_uuid(uuid: Uuid, field_name: &str) -> ValidationResult<()> {
    if uuid.is_nil() {
        return Err(ReputationError::Validation(format!(
            "{} cannot be nil UUID",
            field_name
        )));
    }
    Ok(())
}

/// Validate a percentage value (0-100)
pub fn validate_percentage(value: Decimal) -> ValidationResult<()> {
    if value < Decimal::ZERO || value > Decimal::from(100) {
        return Err(ReputationError::Validation(format!(
            "Percentage must be between 0 and 100, got {}",
            value
        )));
    }
    Ok(())
}

/// Validate a weight value (0-1)
pub fn validate_weight(weight: Decimal) -> ValidationResult<()> {
    if weight < Decimal::ZERO || weight > Decimal::ONE {
        return Err(ReputationError::Validation(format!(
            "Weight must be between 0 and 1, got {}",
            weight
        )));
    }
    Ok(())
}

/// Validate a non-empty string
pub fn validate_non_empty(value: &str, field_name: &str) -> ValidationResult<()> {
    if value.trim().is_empty() {
        return Err(ReputationError::Validation(format!(
            "{} cannot be empty",
            field_name
        )));
    }
    Ok(())
}

/// Validate a positive decimal value
pub fn validate_positive(value: Decimal, field_name: &str) -> ValidationResult<()> {
    if value <= Decimal::ZERO {
        return Err(ReputationError::Validation(format!(
            "{} must be positive, got {}",
            field_name, value
        )));
    }
    Ok(())
}

/// Validate a non-negative decimal value
pub fn validate_non_negative(value: Decimal, field_name: &str) -> ValidationResult<()> {
    if value < Decimal::ZERO {
        return Err(ReputationError::Validation(format!(
            "{} cannot be negative, got {}",
            field_name, value
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_validate_title_valid() {
        assert!(validate_title("Valid Title").is_ok());
        assert!(validate_title("ABC").is_ok()); // Minimum length
    }

    #[test]
    fn test_validate_title_too_short() {
        assert!(validate_title("AB").is_err());
        assert!(validate_title("").is_err());
    }

    #[test]
    fn test_validate_title_too_long() {
        let long_title = "a".repeat(MAX_TITLE_LENGTH + 1);
        assert!(validate_title(&long_title).is_err());
    }

    #[test]
    fn test_validate_description_valid() {
        assert!(validate_description("Valid description").is_ok());
    }

    #[test]
    fn test_validate_description_empty() {
        assert!(validate_description("").is_err());
        assert!(validate_description("   ").is_err());
    }

    #[test]
    fn test_validate_description_too_long() {
        let long_desc = "a".repeat(MAX_DESCRIPTION_LENGTH + 1);
        assert!(validate_description(&long_desc).is_err());
    }

    #[test]
    fn test_validate_deadline_valid() {
        let future_deadline = Utc::now() + chrono::Duration::days(7);
        assert!(validate_deadline(future_deadline).is_ok());
    }

    #[test]
    fn test_validate_deadline_past() {
        let past_deadline = Utc::now() - chrono::Duration::days(1);
        assert!(validate_deadline(past_deadline).is_err());
    }

    #[test]
    fn test_validate_deadline_too_soon() {
        let too_soon = Utc::now() + chrono::Duration::hours(1);
        assert!(validate_deadline(too_soon).is_err());
    }

    #[test]
    fn test_validate_deadline_too_far() {
        let too_far = Utc::now() + chrono::Duration::days(400);
        assert!(validate_deadline(too_far).is_err());
    }

    #[test]
    fn test_validate_rating_valid() {
        assert!(validate_rating(1).is_ok());
        assert!(validate_rating(3).is_ok());
        assert!(validate_rating(5).is_ok());
    }

    #[test]
    fn test_validate_rating_invalid() {
        assert!(validate_rating(0).is_err());
        assert!(validate_rating(6).is_err());
        assert!(validate_rating(-1).is_err());
    }

    #[test]
    fn test_validate_feedback_valid() {
        assert!(validate_feedback("Good work!").is_ok());
        assert!(validate_feedback("").is_ok()); // Empty is ok
    }

    #[test]
    fn test_validate_feedback_too_long() {
        let long_feedback = "a".repeat(MAX_FEEDBACK_LENGTH + 1);
        assert!(validate_feedback(&long_feedback).is_err());
    }

    #[test]
    fn test_validate_score_valid() {
        assert!(validate_score(dec!(0)).is_ok());
        assert!(validate_score(dec!(500)).is_ok());
        assert!(validate_score(dec!(1000)).is_ok());
    }

    #[test]
    fn test_validate_score_invalid() {
        assert!(validate_score(dec!(-1)).is_err());
        assert!(validate_score(dec!(1001)).is_err());
    }

    #[test]
    fn test_validate_uuid_valid() {
        let valid_uuid = Uuid::new_v4();
        assert!(validate_uuid(valid_uuid, "test_field").is_ok());
    }

    #[test]
    fn test_validate_uuid_nil() {
        assert!(validate_uuid(Uuid::nil(), "test_field").is_err());
    }

    #[test]
    fn test_validate_percentage_valid() {
        assert!(validate_percentage(dec!(0)).is_ok());
        assert!(validate_percentage(dec!(50)).is_ok());
        assert!(validate_percentage(dec!(100)).is_ok());
    }

    #[test]
    fn test_validate_percentage_invalid() {
        assert!(validate_percentage(dec!(-1)).is_err());
        assert!(validate_percentage(dec!(101)).is_err());
    }

    #[test]
    fn test_validate_weight_valid() {
        assert!(validate_weight(dec!(0)).is_ok());
        assert!(validate_weight(dec!(0.5)).is_ok());
        assert!(validate_weight(dec!(1)).is_ok());
    }

    #[test]
    fn test_validate_weight_invalid() {
        assert!(validate_weight(dec!(-0.1)).is_err());
        assert!(validate_weight(dec!(1.1)).is_err());
    }

    #[test]
    fn test_validate_non_empty_valid() {
        assert!(validate_non_empty("test", "field").is_ok());
    }

    #[test]
    fn test_validate_non_empty_invalid() {
        assert!(validate_non_empty("", "field").is_err());
        assert!(validate_non_empty("   ", "field").is_err());
    }

    #[test]
    fn test_validate_positive_valid() {
        assert!(validate_positive(dec!(1), "field").is_ok());
        assert!(validate_positive(dec!(100), "field").is_ok());
    }

    #[test]
    fn test_validate_positive_invalid() {
        assert!(validate_positive(dec!(0), "field").is_err());
        assert!(validate_positive(dec!(-1), "field").is_err());
    }

    #[test]
    fn test_validate_non_negative_valid() {
        assert!(validate_non_negative(dec!(0), "field").is_ok());
        assert!(validate_non_negative(dec!(100), "field").is_ok());
    }

    #[test]
    fn test_validate_non_negative_invalid() {
        assert!(validate_non_negative(dec!(-1), "field").is_err());
    }

    // ========================================================================
    // Property-Based Tests
    // ========================================================================

    mod proptest_validation {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: Valid scores (0-1000) should always pass validation
            #[test]
            fn prop_valid_scores_accepted(score in 0i64..=1000i64) {
                let result = validate_score(Decimal::new(score, 0));
                prop_assert!(result.is_ok());
            }

            /// Property: Invalid scores should be rejected
            #[test]
            fn prop_invalid_scores_rejected(score in -1000i64..=-1i64) {
                let result = validate_score(Decimal::new(score, 0));
                prop_assert!(result.is_err());
            }

            /// Property: Scores above 1000 should be rejected
            #[test]
            fn prop_scores_above_max_rejected(score in 1001i64..=10000i64) {
                let result = validate_score(Decimal::new(score, 0));
                prop_assert!(result.is_err());
            }

            /// Property: Valid ratings (1-5) should always pass validation
            #[test]
            fn prop_valid_ratings_accepted(rating in 1i32..=5i32) {
                let result = validate_rating(rating);
                prop_assert!(result.is_ok());
            }

            /// Property: Invalid ratings should be rejected
            #[test]
            fn prop_invalid_ratings_rejected(rating in -100i32..=0i32) {
                let result = validate_rating(rating);
                prop_assert!(result.is_err());
            }

            /// Property: Ratings above 5 should be rejected
            #[test]
            fn prop_ratings_above_max_rejected(rating in 6i32..=100i32) {
                let result = validate_rating(rating);
                prop_assert!(result.is_err());
            }

            /// Property: Valid percentages (0-100) should always pass validation
            #[test]
            fn prop_valid_percentages_accepted(pct in 0i64..=100i64) {
                let result = validate_percentage(Decimal::new(pct, 0));
                prop_assert!(result.is_ok());
            }

            /// Property: Negative percentages should be rejected
            #[test]
            fn prop_negative_percentages_rejected(pct in -100i64..=-1i64) {
                let result = validate_percentage(Decimal::new(pct, 0));
                prop_assert!(result.is_err());
            }

            /// Property: Percentages above 100 should be rejected
            #[test]
            fn prop_percentages_above_max_rejected(pct in 101i64..=1000i64) {
                let result = validate_percentage(Decimal::new(pct, 0));
                prop_assert!(result.is_err());
            }

            /// Property: Valid weights (0-1) should always pass validation
            #[test]
            fn prop_valid_weights_accepted(weight_int in 0i64..=100i64) {
                let weight = Decimal::new(weight_int, 2); // 0.00 to 1.00
                let result = validate_weight(weight);
                prop_assert!(result.is_ok());
            }

            /// Property: Negative weights should be rejected
            #[test]
            fn prop_negative_weights_rejected(weight in -100i64..=-1i64) {
                let result = validate_weight(Decimal::new(weight, 2));
                prop_assert!(result.is_err());
            }

            /// Property: Weights above 1 should be rejected
            #[test]
            fn prop_weights_above_max_rejected(weight_int in 101i64..=1000i64) {
                let weight = Decimal::new(weight_int, 2); // > 1.00
                let result = validate_weight(weight);
                prop_assert!(result.is_err());
            }

            /// Property: Non-empty strings should pass validation
            /// String must have at least one non-whitespace character
            #[test]
            fn prop_non_empty_strings_accepted(s in "[a-zA-Z0-9][a-zA-Z0-9 ]{0,99}") {
                let result = validate_non_empty(&s, "field");
                prop_assert!(result.is_ok());
            }

            /// Property: Positive values should pass validation
            #[test]
            fn prop_positive_values_accepted(val in 1i64..=10000i64) {
                let result = validate_positive(Decimal::new(val, 0), "field");
                prop_assert!(result.is_ok());
            }

            /// Property: Zero and negative values should be rejected as positive
            #[test]
            fn prop_non_positive_values_rejected(val in -1000i64..=0i64) {
                let result = validate_positive(Decimal::new(val, 0), "field");
                prop_assert!(result.is_err());
            }

            /// Property: Non-negative values (including zero) should pass validation
            #[test]
            fn prop_non_negative_values_accepted(val in 0i64..=10000i64) {
                let result = validate_non_negative(Decimal::new(val, 0), "field");
                prop_assert!(result.is_ok());
            }

            /// Property: Negative values should be rejected as non-negative
            #[test]
            fn prop_negative_values_rejected_non_negative(val in -10000i64..=-1i64) {
                let result = validate_non_negative(Decimal::new(val, 0), "field");
                prop_assert!(result.is_err());
            }

            /// Property: Titles within bounds should pass validation
            /// Title must have at least 3 non-whitespace chars after trimming
            #[test]
            fn prop_valid_titles_accepted(s in "[a-zA-Z0-9][a-zA-Z0-9 ]{2,48}[a-zA-Z0-9]") {
                let result = validate_title(&s);
                prop_assert!(result.is_ok());
            }

            /// Property: Descriptions within bounds should pass validation
            /// Description must be non-empty after trimming
            #[test]
            fn prop_valid_descriptions_accepted(s in "[a-zA-Z0-9][a-zA-Z0-9 ]{0,98}[a-zA-Z0-9]?") {
                let result = validate_description(&s);
                prop_assert!(result.is_ok());
            }

            /// Property: Feedback within bounds should pass validation
            #[test]
            fn prop_valid_feedback_accepted(s in "[a-zA-Z0-9 ]{0,100}") {
                let result = validate_feedback(&s);
                prop_assert!(result.is_ok());
            }
        }
    }
}
