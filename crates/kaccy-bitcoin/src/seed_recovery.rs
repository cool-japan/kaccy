//! BIP 39 Mnemonic Seed Phrase Utilities
//!
//! This module provides comprehensive BIP 39 support for generating, validating,
//! and recovering Bitcoin wallet seeds using mnemonic phrases.
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::seed_recovery::{MnemonicGenerator, MnemonicValidator, WordCount};
//!
//! // Generate a 24-word mnemonic
//! let mnemonic = MnemonicGenerator::generate(WordCount::TwentyFour).unwrap();
//! println!("Mnemonic: {}", mnemonic.phrase());
//!
//! // Validate a mnemonic
//! let validator = MnemonicValidator::new();
//! assert!(validator.validate(mnemonic.phrase()).is_ok());
//!
//! // Derive seed from mnemonic (with optional passphrase)
//! let seed = mnemonic.to_seed(Some("my-secret-passphrase"));
//! println!("Seed: {}", hex::encode(&seed));
//! ```

use crate::error::BitcoinError;
use bip39::{Language, Mnemonic as Bip39Mnemonic};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Number of words in a BIP 39 mnemonic phrase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordCount {
    /// 12 words (128 bits of entropy)
    Twelve = 12,
    /// 15 words (160 bits of entropy)
    Fifteen = 15,
    /// 18 words (192 bits of entropy)
    Eighteen = 18,
    /// 21 words (224 bits of entropy)
    TwentyOne = 21,
    /// 24 words (256 bits of entropy)
    TwentyFour = 24,
}

impl WordCount {
    /// Get the number of entropy bits for this word count
    pub fn entropy_bits(&self) -> usize {
        match self {
            WordCount::Twelve => 128,
            WordCount::Fifteen => 160,
            WordCount::Eighteen => 192,
            WordCount::TwentyOne => 224,
            WordCount::TwentyFour => 256,
        }
    }

    /// Get the number of entropy bytes for this word count
    pub fn entropy_bytes(&self) -> usize {
        self.entropy_bits() / 8
    }
}

/// Supported languages for BIP 39 mnemonic phrases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MnemonicLanguage {
    /// English (only language currently supported by bip39 crate)
    English,
}

impl MnemonicLanguage {
    /// Convert to bip39 Language
    fn to_bip39_language(self) -> Language {
        match self {
            MnemonicLanguage::English => Language::English,
        }
    }
}

/// A BIP 39 mnemonic phrase
#[derive(Clone)]
pub struct Mnemonic {
    inner: Bip39Mnemonic,
    language: MnemonicLanguage,
}

impl Mnemonic {
    /// Create a new mnemonic from a phrase string
    pub fn from_phrase(phrase: &str, language: MnemonicLanguage) -> Result<Self, BitcoinError> {
        let inner = Bip39Mnemonic::parse_in(language.to_bip39_language(), phrase)
            .map_err(|e| BitcoinError::InvalidInput(format!("Invalid mnemonic: {}", e)))?;
        Ok(Self { inner, language })
    }

    /// Get the mnemonic phrase as a string
    pub fn phrase(&self) -> &str {
        self.inner.words().collect::<Vec<_>>().join(" ").leak()
    }

    /// Get the word at the specified index (0-based)
    pub fn word_at(&self, index: usize) -> Option<&str> {
        self.inner.words().nth(index)
    }

    /// Get the number of words in this mnemonic
    pub fn word_count(&self) -> usize {
        self.inner.words().count()
    }

    /// Get the language of this mnemonic
    pub fn language(&self) -> MnemonicLanguage {
        self.language
    }

    /// Convert the mnemonic to a seed with optional passphrase
    ///
    /// The passphrase adds an additional layer of security. If no passphrase
    /// is used, pass `None` or `Some("")`.
    pub fn to_seed(&self, passphrase: Option<&str>) -> Vec<u8> {
        self.inner.to_seed(passphrase.unwrap_or("")).to_vec()
    }

    /// Convert the mnemonic to entropy bytes
    pub fn to_entropy(&self) -> Vec<u8> {
        self.inner.to_entropy().to_vec()
    }
}

impl fmt::Debug for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mnemonic")
            .field("word_count", &self.word_count())
            .field("language", &self.language)
            .field("phrase", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.phrase())
    }
}

/// Mnemonic generator for creating new BIP 39 phrases
pub struct MnemonicGenerator {
    #[allow(dead_code)]
    language: MnemonicLanguage,
}

impl MnemonicGenerator {
    /// Create a new mnemonic generator with default language (English)
    pub fn new() -> Self {
        Self {
            language: MnemonicLanguage::English,
        }
    }

    /// Create a new mnemonic generator with specified language
    pub fn with_language(language: MnemonicLanguage) -> Self {
        Self { language }
    }

    /// Generate a new random mnemonic with the specified word count
    pub fn generate(word_count: WordCount) -> Result<Mnemonic, BitcoinError> {
        Self::new().generate_with_language(word_count, MnemonicLanguage::English)
    }

    /// Generate a new random mnemonic with specified word count and language
    pub fn generate_with_language(
        &self,
        word_count: WordCount,
        language: MnemonicLanguage,
    ) -> Result<Mnemonic, BitcoinError> {
        use rand::Rng;
        let mut rng = rand::rng();
        let entropy_bytes = word_count.entropy_bytes();
        let mut entropy = vec![0u8; entropy_bytes];
        rng.fill(&mut entropy[..]);

        let inner = Bip39Mnemonic::from_entropy_in(language.to_bip39_language(), &entropy)
            .map_err(|e| {
                BitcoinError::InvalidInput(format!("Failed to generate mnemonic: {}", e))
            })?;

        Ok(Mnemonic { inner, language })
    }

    /// Generate a mnemonic from provided entropy
    pub fn from_entropy(
        entropy: &[u8],
        language: MnemonicLanguage,
    ) -> Result<Mnemonic, BitcoinError> {
        let inner = Bip39Mnemonic::from_entropy_in(language.to_bip39_language(), entropy)
            .map_err(|e| BitcoinError::InvalidInput(format!("Invalid entropy: {}", e)))?;
        Ok(Mnemonic { inner, language })
    }
}

impl Default for MnemonicGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Mnemonic validator for checking BIP 39 phrase validity
pub struct MnemonicValidator {
    language: MnemonicLanguage,
}

impl MnemonicValidator {
    /// Create a new validator with default language (English)
    pub fn new() -> Self {
        Self {
            language: MnemonicLanguage::English,
        }
    }

    /// Create a new validator with specified language
    pub fn with_language(language: MnemonicLanguage) -> Self {
        Self { language }
    }

    /// Validate a mnemonic phrase
    pub fn validate(&self, phrase: &str) -> Result<(), BitcoinError> {
        Bip39Mnemonic::parse_in(self.language.to_bip39_language(), phrase)
            .map_err(|e| BitcoinError::InvalidInput(format!("Invalid mnemonic: {}", e)))?;
        Ok(())
    }

    /// Validate and parse a mnemonic phrase
    pub fn validate_and_parse(&self, phrase: &str) -> Result<Mnemonic, BitcoinError> {
        Mnemonic::from_phrase(phrase, self.language)
    }

    /// Check if a word is in the BIP 39 wordlist
    pub fn is_valid_word(&self, word: &str) -> bool {
        let language = self.language.to_bip39_language();
        language.word_list().iter().any(|w| w == &word)
    }

    /// Get all possible words that start with the given prefix
    pub fn autocomplete(&self, prefix: &str) -> Vec<&'static str> {
        let language = self.language.to_bip39_language();
        language
            .word_list()
            .iter()
            .filter(|w| w.starts_with(prefix))
            .copied()
            .collect()
    }
}

impl Default for MnemonicValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// XOR two mnemonics for split backup (Seed XOR)
///
/// This allows you to split a seed into multiple parts where both parts
/// are needed to reconstruct the original seed.
pub struct SeedXor;

impl SeedXor {
    /// XOR two entropy values to create a split backup
    pub fn xor_entropy(a: &[u8], b: &[u8]) -> Result<Vec<u8>, BitcoinError> {
        if a.len() != b.len() {
            return Err(BitcoinError::InvalidInput(
                "Entropy lengths must match".to_string(),
            ));
        }

        Ok(a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect())
    }

    /// XOR two mnemonics (they must have the same word count)
    pub fn xor_mnemonics(a: &Mnemonic, b: &Mnemonic) -> Result<Mnemonic, BitcoinError> {
        let entropy_a = a.to_entropy();
        let entropy_b = b.to_entropy();

        let xored = Self::xor_entropy(&entropy_a, &entropy_b)?;
        MnemonicGenerator::from_entropy(&xored, a.language())
    }
}

/// Statistics about mnemonic generation and validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MnemonicStats {
    /// Number of mnemonics generated
    pub generated_count: u64,
    /// Number of mnemonics validated
    pub validated_count: u64,
    /// Number of validation failures
    pub validation_failures: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_count_entropy() {
        assert_eq!(WordCount::Twelve.entropy_bits(), 128);
        assert_eq!(WordCount::Twelve.entropy_bytes(), 16);
        assert_eq!(WordCount::TwentyFour.entropy_bits(), 256);
        assert_eq!(WordCount::TwentyFour.entropy_bytes(), 32);
    }

    #[test]
    fn test_generate_mnemonic_12_words() {
        let mnemonic = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        assert_eq!(mnemonic.word_count(), 12);
    }

    #[test]
    fn test_generate_mnemonic_24_words() {
        let mnemonic = MnemonicGenerator::generate(WordCount::TwentyFour).unwrap();
        assert_eq!(mnemonic.word_count(), 24);
    }

    #[test]
    fn test_validate_valid_mnemonic() {
        let mnemonic = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        let validator = MnemonicValidator::new();
        assert!(validator.validate(mnemonic.phrase()).is_ok());
    }

    #[test]
    fn test_validate_invalid_mnemonic() {
        let validator = MnemonicValidator::new();
        let result = validator.validate("invalid mnemonic phrase words test fail");
        assert!(result.is_err());
    }

    #[test]
    fn test_mnemonic_to_seed() {
        let mnemonic = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        let seed = mnemonic.to_seed(None);
        assert_eq!(seed.len(), 64); // BIP 39 seeds are always 64 bytes
    }

    #[test]
    fn test_mnemonic_to_seed_with_passphrase() {
        let mnemonic = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        let seed1 = mnemonic.to_seed(None);
        let seed2 = mnemonic.to_seed(Some("passphrase"));
        assert_ne!(seed1, seed2); // Different passphrases should produce different seeds
    }

    #[test]
    fn test_mnemonic_word_at() {
        let mnemonic = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        assert!(mnemonic.word_at(0).is_some());
        assert!(mnemonic.word_at(11).is_some());
        assert!(mnemonic.word_at(12).is_none());
    }

    #[test]
    fn test_is_valid_word() {
        let validator = MnemonicValidator::new();
        assert!(validator.is_valid_word("abandon"));
        assert!(validator.is_valid_word("zoo"));
        assert!(!validator.is_valid_word("invalid"));
        assert!(!validator.is_valid_word("notaword"));
    }

    #[test]
    fn test_autocomplete() {
        let validator = MnemonicValidator::new();
        let words = validator.autocomplete("aba");
        assert!(words.contains(&"abandon"));
        assert!(!words.is_empty());
    }

    #[test]
    fn test_mnemonic_from_phrase() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::from_phrase(phrase, MnemonicLanguage::English).unwrap();
        assert_eq!(mnemonic.word_count(), 12);
    }

    #[test]
    fn test_seed_xor() {
        let a = vec![0x11, 0x22, 0x33, 0x44];
        let b = vec![0xFF, 0xEE, 0xDD, 0xCC];
        let result = SeedXor::xor_entropy(&a, &b).unwrap();
        assert_eq!(result, vec![0xEE, 0xCC, 0xEE, 0x88]);
    }

    #[test]
    fn test_seed_xor_length_mismatch() {
        let a = vec![0x11, 0x22];
        let b = vec![0xFF, 0xEE, 0xDD];
        let result = SeedXor::xor_entropy(&a, &b);
        assert!(result.is_err());
    }

    #[test]
    fn test_xor_mnemonics() {
        let mnemonic_a = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        let mnemonic_b = MnemonicGenerator::generate(WordCount::Twelve).unwrap();
        let result = SeedXor::xor_mnemonics(&mnemonic_a, &mnemonic_b);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().word_count(), 12);
    }

    #[test]
    fn test_from_entropy() {
        let entropy = vec![0x11; 16]; // 128 bits for 12 words
        let mnemonic =
            MnemonicGenerator::from_entropy(&entropy, MnemonicLanguage::English).unwrap();
        assert_eq!(mnemonic.word_count(), 12);
    }

    #[test]
    fn test_to_entropy() {
        let entropy = vec![0x11; 16];
        let mnemonic =
            MnemonicGenerator::from_entropy(&entropy, MnemonicLanguage::English).unwrap();
        let recovered_entropy = mnemonic.to_entropy();
        assert_eq!(entropy, recovered_entropy);
    }
}
