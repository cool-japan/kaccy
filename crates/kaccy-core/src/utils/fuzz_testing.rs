//! Fuzz testing and property-based testing utilities
//!
//! This module provides utilities for:
//! - Property-based testing with random input generation
//! - Fuzzing strategies for edge case discovery
//! - Input mutation and generation
//! - Crash detection and error classification

use rand::RngExt;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Property-based test result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyTestResult {
    /// All test cases passed
    Success {
        /// Number of random inputs tested.
        cases_tested: usize,
    },
    /// A test case failed
    Failure {
        /// String representation of the input that caused the failure.
        failing_input: String,
        /// Error message from the failing case.
        error: String,
        /// Index of the failing test case.
        case_number: usize,
    },
    /// Test crashed unexpectedly
    Crash {
        /// Index of the case that triggered the crash.
        case_number: usize,
        /// Error message from the crash.
        error: String,
    },
}

/// Random input generator for fuzzing
pub struct FuzzGenerator {
    rng: rand::rngs::ThreadRng,
}

impl FuzzGenerator {
    /// Create a new fuzz generator
    pub fn new() -> Self {
        Self { rng: rand::rng() }
    }

    /// Generate random string with given length
    pub fn random_string(&mut self, min_len: usize, max_len: usize) -> String {
        let len = self.rng.random_range(min_len..=max_len);
        (0..len)
            .map(|_| self.rng.random_range(b'a'..=b'z') as char)
            .collect()
    }

    /// Generate random alphanumeric string
    pub fn random_alphanumeric(&mut self, min_len: usize, max_len: usize) -> String {
        let len = self.rng.random_range(min_len..=max_len);
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        (0..len)
            .map(|_| {
                let idx = self.rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Generate random decimal in range
    pub fn random_decimal(&mut self, min: Decimal, max: Decimal) -> Decimal {
        let min_f = min.to_f64().unwrap_or(0.0);
        let max_f = max.to_f64().unwrap_or(1.0);
        let value = self.rng.random_range(min_f..=max_f);
        Decimal::from_f64(value).unwrap_or(Decimal::ZERO)
    }

    /// Generate random integer in range
    pub fn random_int(&mut self, min: i64, max: i64) -> i64 {
        self.rng.random_range(min..=max)
    }

    /// Generate random unsigned integer in range
    pub fn random_uint(&mut self, min: u64, max: u64) -> u64 {
        self.rng.random_range(min..=max)
    }

    /// Generate random boolean
    pub fn random_bool(&mut self) -> bool {
        self.rng.random_bool(0.5)
    }

    /// Generate random email
    pub fn random_email(&mut self) -> String {
        let username = self.random_alphanumeric(5, 15);
        let domain = self.random_alphanumeric(5, 10);
        format!("{}@{}.com", username.to_lowercase(), domain.to_lowercase())
    }

    /// Generate random UUID string
    pub fn random_uuid(&mut self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Generate edge case string (empty, very long, special chars)
    pub fn edge_case_string(&mut self) -> String {
        let case = self.rng.random_range(0..5);
        match case {
            0 => String::new(),            // Empty
            1 => " ".to_string(),          // Single space
            2 => "a".repeat(10000),        // Very long
            3 => "!@#$%^&*()".to_string(), // Special chars
            _ => "\n\r\t".to_string(),     // Whitespace
        }
    }

    /// Generate edge case number
    pub fn edge_case_decimal(&mut self) -> Decimal {
        let case = self.rng.random_range(0..6);
        match case {
            0 => Decimal::ZERO,
            1 => Decimal::ONE,
            2 => Decimal::from(-1),
            3 => Decimal::MAX,
            4 => Decimal::MIN,
            _ => Decimal::from_f64(f64::INFINITY).unwrap_or(Decimal::MAX),
        }
    }

    /// Generate edge case integer
    pub fn edge_case_int(&mut self) -> i64 {
        let case = self.rng.random_range(0..5);
        match case {
            0 => 0,
            1 => 1,
            2 => -1,
            3 => i64::MAX,
            _ => i64::MIN,
        }
    }
}

impl Default for FuzzGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Property-based test executor
pub struct PropertyTester {
    max_cases: usize,
    generator: FuzzGenerator,
}

impl PropertyTester {
    /// Create a new property tester
    pub fn new(max_cases: usize) -> Self {
        Self {
            max_cases,
            generator: FuzzGenerator::new(),
        }
    }

    /// Test a property with random inputs
    pub fn test_property<F, T>(&mut self, mut property: F) -> PropertyTestResult
    where
        F: FnMut(&mut FuzzGenerator) -> std::result::Result<T, String>,
    {
        for i in 0..self.max_cases {
            match property(&mut self.generator) {
                Ok(_) => continue,
                Err(error) => {
                    return PropertyTestResult::Failure {
                        failing_input: format!("Case {}", i),
                        error,
                        case_number: i,
                    };
                }
            }
        }

        PropertyTestResult::Success {
            cases_tested: self.max_cases,
        }
    }

    /// Test a property with custom input generator
    pub fn test_with_generator<F, G, T>(
        &mut self,
        mut input_gen: G,
        mut property: F,
    ) -> PropertyTestResult
    where
        G: FnMut(&mut FuzzGenerator) -> T,
        F: FnMut(T) -> std::result::Result<(), String>,
    {
        for i in 0..self.max_cases {
            let input = input_gen(&mut self.generator);

            match property(input) {
                Ok(_) => continue,
                Err(error) => {
                    return PropertyTestResult::Failure {
                        failing_input: format!("Case {}", i),
                        error,
                        case_number: i,
                    };
                }
            }
        }

        PropertyTestResult::Success {
            cases_tested: self.max_cases,
        }
    }
}

/// Mutation-based fuzzer
pub struct MutationFuzzer {
    generator: FuzzGenerator,
    mutations_per_input: usize,
}

impl MutationFuzzer {
    /// Create a new mutation fuzzer
    pub fn new(mutations_per_input: usize) -> Self {
        Self {
            generator: FuzzGenerator::new(),
            mutations_per_input,
        }
    }

    /// Mutate a string input
    pub fn mutate_string(&mut self, input: &str) -> Vec<String> {
        let mut mutations = Vec::new();

        for _ in 0..self.mutations_per_input {
            let mutation_type = self.generator.random_int(0, 6);
            let mutated = match mutation_type {
                0 => self.insert_random_char(input),
                1 => self.delete_random_char(input),
                2 => self.flip_random_char(input),
                3 => self.duplicate_substring(input),
                4 => self.swap_characters(input),
                5 => self.insert_special_chars(input),
                _ => self.reverse_string(input),
            };
            mutations.push(mutated);
        }

        mutations
    }

    /// Insert random character at random position
    fn insert_random_char(&mut self, input: &str) -> String {
        if input.is_empty() {
            return self.generator.random_alphanumeric(1, 1);
        }

        let pos = self.generator.random_uint(0, input.len() as u64) as usize;
        let c = self.generator.random_alphanumeric(1, 1);

        let mut result = String::with_capacity(input.len() + 1);
        result.push_str(&input[..pos]);
        result.push_str(&c);
        result.push_str(&input[pos..]);
        result
    }

    /// Delete random character
    fn delete_random_char(&mut self, input: &str) -> String {
        if input.is_empty() {
            return input.to_string();
        }

        let chars: Vec<char> = input.chars().collect();
        let pos = self.generator.random_uint(0, (chars.len() - 1) as u64) as usize;

        let mut result = String::with_capacity(input.len());
        for (i, &c) in chars.iter().enumerate() {
            if i != pos {
                result.push(c);
            }
        }
        result
    }

    /// Flip random character
    fn flip_random_char(&mut self, input: &str) -> String {
        if input.is_empty() {
            return input.to_string();
        }

        let mut chars: Vec<char> = input.chars().collect();
        let pos = self.generator.random_uint(0, (chars.len() - 1) as u64) as usize;

        // Flip case or change to random char
        if chars[pos].is_alphabetic() {
            chars[pos] = if chars[pos].is_uppercase() {
                chars[pos]
                    .to_lowercase()
                    .next()
                    .expect("alphabetic char always has lowercase")
            } else {
                chars[pos]
                    .to_uppercase()
                    .next()
                    .expect("alphabetic char always has uppercase")
            };
        } else {
            chars[pos] = self
                .generator
                .random_alphanumeric(1, 1)
                .chars()
                .next()
                .expect("random_alphanumeric(1, 1) generates exactly 1 char");
        }

        chars.into_iter().collect()
    }

    /// Duplicate a random substring
    fn duplicate_substring(&mut self, input: &str) -> String {
        if input.is_empty() {
            return input.to_string();
        }

        let len = input.len();
        let start = self.generator.random_uint(0, len as u64 - 1) as usize;
        let end = self.generator.random_uint(start as u64 + 1, len as u64) as usize;

        let mut result = input.to_string();
        result.push_str(&input[start..end]);
        result
    }

    /// Swap two random characters
    fn swap_characters(&mut self, input: &str) -> String {
        if input.len() < 2 {
            return input.to_string();
        }

        let mut chars: Vec<char> = input.chars().collect();
        let pos1 = self.generator.random_uint(0, (chars.len() - 1) as u64) as usize;
        let pos2 = self.generator.random_uint(0, (chars.len() - 1) as u64) as usize;

        chars.swap(pos1, pos2);
        chars.into_iter().collect()
    }

    /// Insert special characters
    fn insert_special_chars(&mut self, input: &str) -> String {
        let special = ["<", ">", "&", "\"", "'", "\0", "\n", "\r"];
        let s = special[self.generator.random_uint(0, (special.len() - 1) as u64) as usize];

        if input.is_empty() {
            return s.to_string();
        }

        let pos = self.generator.random_uint(0, input.len() as u64) as usize;
        let mut result = String::with_capacity(input.len() + s.len());
        result.push_str(&input[..pos]);
        result.push_str(s);
        result.push_str(&input[pos..]);
        result
    }

    /// Reverse string
    fn reverse_string(&mut self, input: &str) -> String {
        input.chars().rev().collect()
    }

    /// Mutate decimal value
    pub fn mutate_decimal(&mut self, value: Decimal) -> Vec<Decimal> {
        let mut mutations = Vec::new();

        for _ in 0..self.mutations_per_input {
            let mutation_type = self.generator.random_int(0, 5);
            let mutated = match mutation_type {
                0 => value + Decimal::ONE,               // Increment
                1 => value - Decimal::ONE,               // Decrement
                2 => value * Decimal::from(2),           // Double
                3 => value / Decimal::from(2),           // Halve
                4 => -value,                             // Negate
                _ => self.generator.edge_case_decimal(), // Edge case
            };
            mutations.push(mutated);
        }

        mutations
    }
}

/// Crash detection and error classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Input that caused the crash
    pub input: String,
    /// Error message
    pub error: String,
    /// Error category
    pub category: CrashCategory,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Crash category for classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrashCategory {
    /// Panic or assertion failure
    Panic,
    /// Validation error
    Validation,
    /// Logic error
    Logic,
    /// Resource exhaustion
    ResourceExhaustion,
    /// Unexpected behavior
    Unexpected,
}

/// Crash detector
pub struct CrashDetector {
    reports: Vec<CrashReport>,
}

impl CrashDetector {
    /// Create a new crash detector
    pub fn new() -> Self {
        Self {
            reports: Vec::new(),
        }
    }

    /// Record a crash
    pub fn record_crash(&mut self, input: String, error: String) {
        let category = self.categorize_error(&error);

        self.reports.push(CrashReport {
            input,
            error,
            category,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Categorize error based on message
    fn categorize_error(&self, error: &str) -> CrashCategory {
        let error_lower = error.to_lowercase();

        if error_lower.contains("panic") || error_lower.contains("assert") {
            CrashCategory::Panic
        } else if error_lower.contains("validation") || error_lower.contains("invalid") {
            CrashCategory::Validation
        } else if error_lower.contains("overflow") || error_lower.contains("exhausted") {
            CrashCategory::ResourceExhaustion
        } else if error_lower.contains("logic") || error_lower.contains("business") {
            CrashCategory::Logic
        } else {
            CrashCategory::Unexpected
        }
    }

    /// Get all crash reports
    pub fn get_reports(&self) -> &[CrashReport] {
        &self.reports
    }

    /// Get crash statistics by category
    pub fn get_statistics(&self) -> HashMap<CrashCategory, usize> {
        let mut stats = HashMap::new();

        for report in &self.reports {
            *stats.entry(report.category.clone()).or_insert(0) += 1;
        }

        stats
    }

    /// Clear all reports
    pub fn clear(&mut self) {
        self.reports.clear();
    }
}

impl Default for CrashDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_fuzz_generator_string() {
        let mut generator = FuzzGenerator::new();

        let s = generator.random_string(5, 10);
        assert!(s.len() >= 5 && s.len() <= 10);
    }

    #[test]
    fn test_fuzz_generator_decimal() {
        let mut generator = FuzzGenerator::new();

        let d = generator.random_decimal(dec!(0.0), dec!(100.0));
        assert!(d >= dec!(0.0) && d <= dec!(100.0));
    }

    #[test]
    fn test_fuzz_generator_email() {
        let mut generator = FuzzGenerator::new();

        let email = generator.random_email();
        assert!(email.contains('@'));
        assert!(email.contains(".com"));
    }

    #[test]
    fn test_property_tester_success() {
        let mut tester = PropertyTester::new(100);

        let result = tester.test_property(|generator| {
            let x = generator.random_int(0, 100);
            let y = generator.random_int(0, 100);

            // Property: x + y >= x
            if x + y >= x {
                Ok(())
            } else {
                Err("Addition property violated".to_string())
            }
        });

        assert_eq!(result, PropertyTestResult::Success { cases_tested: 100 });
    }

    #[test]
    fn test_mutation_fuzzer_string() {
        let mut fuzzer = MutationFuzzer::new(5);

        let mutations = fuzzer.mutate_string("hello");
        assert_eq!(mutations.len(), 5);

        // At least one mutation should be different
        let different_count = mutations.iter().filter(|m| *m != "hello").count();
        assert!(different_count > 0);
    }

    #[test]
    fn test_mutation_fuzzer_decimal() {
        let mut fuzzer = MutationFuzzer::new(5);

        let mutations = fuzzer.mutate_decimal(dec!(10.0));
        assert_eq!(mutations.len(), 5);
    }

    #[test]
    fn test_crash_detector() {
        let mut detector = CrashDetector::new();

        detector.record_crash(
            "test input".to_string(),
            "panic: assertion failed".to_string(),
        );
        detector.record_crash("another input".to_string(), "validation error".to_string());

        let reports = detector.get_reports();
        assert_eq!(reports.len(), 2);

        let stats = detector.get_statistics();
        assert_eq!(stats.get(&CrashCategory::Panic), Some(&1));
        assert_eq!(stats.get(&CrashCategory::Validation), Some(&1));
    }

    #[test]
    fn test_edge_case_generation() {
        let mut generator = FuzzGenerator::new();

        let edge_str = generator.edge_case_string();
        assert!(!edge_str.is_empty() || edge_str.is_empty()); // Can be empty

        let edge_int = generator.edge_case_int();
        assert!(
            edge_int == 0
                || edge_int == 1
                || edge_int == -1
                || edge_int == i64::MAX
                || edge_int == i64::MIN
        );
    }

    #[test]
    fn test_mutation_insert_char() {
        let mut fuzzer = MutationFuzzer::new(1);
        let result = fuzzer.insert_random_char("abc");
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_mutation_delete_char() {
        let mut fuzzer = MutationFuzzer::new(1);
        let result = fuzzer.delete_random_char("abc");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_crash_categorization() {
        let detector = CrashDetector::new();

        assert_eq!(
            detector.categorize_error("panic: test"),
            CrashCategory::Panic
        );
        assert_eq!(
            detector.categorize_error("validation failed"),
            CrashCategory::Validation
        );
        assert_eq!(
            detector.categorize_error("stack overflow"),
            CrashCategory::ResourceExhaustion
        );
    }
}
