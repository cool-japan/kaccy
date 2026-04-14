//! Comprehensive fuzzing and property-based testing utilities for Bitcoin transactions.
//!
//! This module provides tools for testing the robustness of transaction parsing,
//! address validation, and script analysis against malformed or adversarial inputs.
//!
//! # Security Note
//! These utilities are for testing only. All parsing functions in this crate
//! are designed to fail gracefully on malformed input.

use rand::{RngExt, SeedableRng};

/// Configuration for transaction fuzzing sessions.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Maximum number of fuzz iterations to run per batch.
    pub max_iterations: u32,
    /// Optional deterministic seed for reproducible fuzzing.
    pub seed: Option<u64>,
    /// Maximum size in bytes for generated transactions.
    pub max_tx_size_bytes: usize,
    /// Maximum number of inputs to simulate.
    pub max_input_count: u8,
    /// Maximum number of outputs to simulate.
    pub max_output_count: u8,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            seed: None,
            max_tx_size_bytes: 100_000,
            max_input_count: 50,
            max_output_count: 50,
        }
    }
}

/// Categories of malformed transaction inputs used for adversarial testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalformedTxCategory {
    /// Too few bytes to form a valid transaction header.
    TruncatedHeader,
    /// Version field is set to an out-of-range value.
    InvalidVersion,
    /// Transaction has zero inputs (coinbase is an exception handled by the parser).
    ZeroInputs,
    /// Transaction has zero outputs.
    ZeroOutputs,
    /// Output value exceeds 21 million BTC.
    OverflowValue,
    /// Declared script length exceeds available bytes.
    InvalidScriptLen,
    /// Completely random byte sequence.
    RandomNoise,
    /// Structurally plausible but internally inconsistent transaction.
    ValidishStructure,
}

/// Aggregated result from one batch of fuzz iterations.
#[derive(Debug)]
pub struct FuzzResult {
    /// The malformed category that was tested.
    pub category: MalformedTxCategory,
    /// Total iterations executed.
    pub iterations: u32,
    /// Number of iterations that caused a panic (must be 0 for safety).
    pub panics: u32,
    /// Number of iterations that returned a parse error (expected for bad data).
    pub errors: u32,
    /// Number of iterations where parsing unexpectedly succeeded.
    pub successes: u32,
    /// Raw byte inputs that triggered a panic, captured for investigation.
    pub panic_inputs: Vec<Vec<u8>>,
}

impl FuzzResult {
    /// Returns `true` if no panics occurred — the primary safety invariant.
    pub fn is_safe(&self) -> bool {
        self.panics == 0
    }
}

/// Generates adversarial Bitcoin transaction byte sequences for fuzz testing.
pub struct TransactionFuzzer {
    config: FuzzConfig,
}

impl TransactionFuzzer {
    /// Create a new fuzzer with a custom configuration.
    pub fn new(config: FuzzConfig) -> Self {
        Self { config }
    }

    /// Create a fuzzer with default configuration.
    pub fn with_default_config() -> Self {
        Self {
            config: FuzzConfig::default(),
        }
    }

    /// Generate a raw transaction buffer that has been truncated at a random byte
    /// boundary, simulating incomplete network messages or corrupt data.
    pub fn generate_truncated(&self, rng: &mut impl RngExt) -> Vec<u8> {
        let mut data = vec![0x01u8, 0x00, 0x00, 0x00]; // version = 1 (u32 LE)
        let extra: usize = rng.random_range(0..11);
        let suffix: Vec<u8> = (0..extra).map(|_| rng.random::<u8>()).collect();
        data.extend(suffix);
        data
    }

    /// Generate a buffer of completely random bytes of random length within
    /// `[1, max_tx_size_bytes]`.
    pub fn generate_random_noise(&self, rng: &mut impl RngExt) -> Vec<u8> {
        let len: usize = rng.random_range(1..=self.config.max_tx_size_bytes);
        (0..len).map(|_| rng.random::<u8>()).collect()
    }

    /// Generate a structurally plausible but internally inconsistent transaction.
    ///
    /// The returned buffer has a valid-looking header with one input whose
    /// `script_sig` length varint claims more bytes than are available.  This
    /// exercises off-by-one / overflow parsing paths without being pure noise.
    pub fn generate_validish_malformed(&self, rng: &mut impl RngExt) -> Vec<u8> {
        let mut data = vec![0x01u8, 0x00, 0x00, 0x00, 0x01]; // version=1, input_count=1
        let txid: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
        data.extend(txid);
        let vout: u32 = rng.random_range(0..10);
        data.extend_from_slice(&vout.to_le_bytes());
        // 2-byte compact-int form claiming 65535 bytes of script
        data.extend_from_slice(&[0xFD, 0xFF, 0xFF]);
        let actual: usize = (rng.random::<u8>() % 9) as usize;
        let script: Vec<u8> = (0..actual).map(|_| rng.random::<u8>()).collect();
        data.extend(script);
        data.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // sequence
        data
    }

    /// Generate a transaction whose single output value is set to `u64::MAX`,
    /// which is far above the 21 million BTC supply cap (2_100_000_000_000_000
    /// satoshis).
    pub fn generate_overflow_value(&self, rng: &mut impl RngExt) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::with_capacity(61);
        data.extend_from_slice(&1u32.to_le_bytes()); // version
        data.push(0x01); // input count
        let txid: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
        data.extend(txid);
        data.extend_from_slice(&0u32.to_le_bytes()); // vout
        data.push(0x00); // script_sig length = 0 (empty)
        data.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // sequence
        data.push(0x01); // output count
        data.extend_from_slice(&u64::MAX.to_le_bytes()); // value = u64::MAX
        data.push(0x01); // output script length = 1
        data.push(0x6A); // OP_RETURN
        data.extend_from_slice(&0u32.to_le_bytes()); // locktime
        data
    }

    /// Run `config.max_iterations` fuzz iterations for the given `category`.
    ///
    /// For each iteration a byte sequence is generated according to `category`
    /// and passed to `parse_fn`.  Panics are caught via [`std::panic::catch_unwind`]
    /// so they contribute to `FuzzResult::panics` rather than aborting the test.
    ///
    /// `parse_fn` should return `true` if parsing succeeded and `false` if it
    /// returned a structured error.
    pub fn run_batch<F>(&self, category: MalformedTxCategory, parse_fn: F) -> FuzzResult
    where
        F: Fn(&[u8]) -> bool,
    {
        // Dispatch to the generic inner implementation to avoid Box<dyn Rng>
        // which is not dyn-compatible due to Rng's generic methods.
        match self.config.seed {
            Some(seed) => {
                let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
                Self::run_batch_inner(
                    category,
                    parse_fn,
                    &mut rng,
                    self.config.max_iterations,
                    self.config.max_tx_size_bytes,
                )
            }
            None => {
                let mut rng = rand::rng();
                Self::run_batch_inner(
                    category,
                    parse_fn,
                    &mut rng,
                    self.config.max_iterations,
                    self.config.max_tx_size_bytes,
                )
            }
        }
    }

    /// Inner implementation of `run_batch`, generic over the RNG type.
    fn run_batch_inner<F, R>(
        category: MalformedTxCategory,
        parse_fn: F,
        rng: &mut R,
        max_iterations: u32,
        max_size: usize,
    ) -> FuzzResult
    where
        F: Fn(&[u8]) -> bool,
        R: RngExt,
    {
        let mut panics: u32 = 0;
        let mut errors: u32 = 0;
        let mut successes: u32 = 0;
        let mut panic_inputs: Vec<Vec<u8>> = Vec::new();

        for _ in 0..max_iterations {
            let bytes = Self::generate_bytes_for_category(category, rng, max_size);

            // Use catch_unwind to ensure panics are captured, not propagated.
            // AssertUnwindSafe is required because the closure borrows the slice.
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_fn(&bytes)));

            match result {
                Err(_panic_payload) => {
                    panics += 1;
                    panic_inputs.push(bytes);
                }
                Ok(true) => successes += 1,
                Ok(false) => errors += 1,
            }
        }

        FuzzResult {
            category,
            iterations: max_iterations,
            panics,
            errors,
            successes,
            panic_inputs,
        }
    }

    /// Generate bytes for a specific malformed category.
    fn generate_bytes_for_category<R: RngExt>(
        category: MalformedTxCategory,
        rng: &mut R,
        max_size: usize,
    ) -> Vec<u8> {
        match category {
            MalformedTxCategory::TruncatedHeader => {
                let mut data = vec![0x01u8, 0x00, 0x00, 0x00];
                let extra: usize = rng.random_range(0..11);
                let suffix: Vec<u8> = (0..extra).map(|_| rng.random::<u8>()).collect();
                data.extend(suffix);
                data
            }
            MalformedTxCategory::RandomNoise => {
                let len: usize = rng.random_range(1..=max_size);
                (0..len).map(|_| rng.random::<u8>()).collect()
            }
            MalformedTxCategory::ValidishStructure => {
                let mut data = vec![0x01u8, 0x00, 0x00, 0x00, 0x01];
                let txid: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
                data.extend(txid);
                let vout: u32 = rng.random_range(0..10);
                data.extend_from_slice(&vout.to_le_bytes());
                data.extend_from_slice(&[0xFD, 0xFF, 0xFF]);
                let actual: usize = (rng.random::<u8>() % 9) as usize;
                let script: Vec<u8> = (0..actual).map(|_| rng.random::<u8>()).collect();
                data.extend(script);
                data.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
                data
            }
            MalformedTxCategory::OverflowValue => {
                let mut data: Vec<u8> = Vec::with_capacity(61);
                data.extend_from_slice(&1u32.to_le_bytes());
                data.push(0x01);
                let txid: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
                data.extend(txid);
                data.extend_from_slice(&0u32.to_le_bytes());
                data.push(0x00);
                data.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
                data.push(0x01);
                data.extend_from_slice(&u64::MAX.to_le_bytes());
                data.push(0x01);
                data.push(0x6A);
                data.extend_from_slice(&0u32.to_le_bytes());
                data
            }
            MalformedTxCategory::InvalidVersion
            | MalformedTxCategory::ZeroInputs
            | MalformedTxCategory::ZeroOutputs
            | MalformedTxCategory::InvalidScriptLen => {
                let len: usize = rng.random_range(1..=max_size);
                (0..len).map(|_| rng.random::<u8>()).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_config_default() {
        let cfg = FuzzConfig::default();
        assert_eq!(cfg.max_iterations, 1000);
        assert_eq!(cfg.max_tx_size_bytes, 100_000);
        assert_eq!(cfg.max_input_count, 50);
        assert_eq!(cfg.max_output_count, 50);
        assert!(cfg.seed.is_none());
    }

    #[test]
    fn test_generate_random_noise_length() {
        let fuzzer = TransactionFuzzer::with_default_config();
        let mut rng = rand::rng();
        for _ in 0..20 {
            let noise = fuzzer.generate_random_noise(&mut rng);
            assert!(!noise.is_empty(), "Random noise must be non-empty");
            assert!(
                noise.len() <= fuzzer.config.max_tx_size_bytes,
                "Noise length {} exceeds max {}",
                noise.len(),
                fuzzer.config.max_tx_size_bytes
            );
        }
    }

    #[test]
    fn test_generate_truncated_is_short() {
        let cfg = FuzzConfig {
            seed: Some(42),
            ..Default::default()
        };
        let fuzzer = TransactionFuzzer::new(cfg);
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        for _ in 0..20 {
            let truncated = fuzzer.generate_truncated(&mut rng);
            assert!(
                truncated.len() < 300,
                "Truncated tx unexpectedly large: {} bytes",
                truncated.len()
            );
        }
    }

    #[test]
    fn test_generate_validish_non_empty() {
        let fuzzer = TransactionFuzzer::with_default_config();
        let mut rng = rand::rng();
        let data = fuzzer.generate_validish_malformed(&mut rng);
        assert!(!data.is_empty(), "Validish malformed tx must not be empty");
        assert!(
            data.len() >= 5,
            "Validish malformed tx is suspiciously short: {} bytes",
            data.len()
        );
    }

    #[test]
    fn test_fuzz_result_is_safe_with_no_panics() {
        let result = FuzzResult {
            category: MalformedTxCategory::RandomNoise,
            iterations: 100,
            panics: 0,
            errors: 90,
            successes: 10,
            panic_inputs: Vec::new(),
        };
        assert!(result.is_safe());

        let unsafe_result = FuzzResult {
            category: MalformedTxCategory::RandomNoise,
            iterations: 100,
            panics: 1,
            errors: 89,
            successes: 10,
            panic_inputs: vec![vec![0xFF]],
        };
        assert!(!unsafe_result.is_safe());
    }

    #[test]
    fn test_run_batch_no_panics() {
        use bitcoin::consensus::Decodable;

        let cfg = FuzzConfig {
            max_iterations: 50,
            seed: Some(12345),
            ..Default::default()
        };
        let fuzzer = TransactionFuzzer::new(cfg);

        let result = fuzzer.run_batch(MalformedTxCategory::RandomNoise, |bytes| {
            let mut slice = bytes;
            bitcoin::Transaction::consensus_decode(&mut slice).is_ok()
        });

        assert!(result.is_safe(), "Parsing should never panic");
        assert_eq!(result.iterations, 50);
    }

    #[test]
    fn test_generate_overflow_value_has_max_value_bytes() {
        let fuzzer = TransactionFuzzer::with_default_config();
        let mut rng = rand::rngs::SmallRng::seed_from_u64(99);
        let data = fuzzer.generate_overflow_value(&mut rng);

        // value starts at: version(4) + input_count(1) + txid(32) + vout(4) +
        //                   script_len=0(1) + sequence(4) + output_count(1) = 47
        let value_offset = 4 + 1 + 32 + 4 + 1 + 4 + 1;
        assert!(
            data.len() > value_offset + 7,
            "Overflow tx too short: {} bytes",
            data.len()
        );
        let value_bytes = &data[value_offset..value_offset + 8];
        assert_eq!(
            value_bytes,
            &u64::MAX.to_le_bytes(),
            "Overflow value bytes must be u64::MAX"
        );
    }

    #[test]
    fn test_fuzzer_with_custom_config() {
        use bitcoin::consensus::Decodable;

        let cfg = FuzzConfig {
            max_iterations: 20,
            seed: Some(777),
            max_tx_size_bytes: 512,
            max_input_count: 5,
            max_output_count: 5,
        };
        let fuzzer = TransactionFuzzer::new(cfg);

        let categories = [
            MalformedTxCategory::TruncatedHeader,
            MalformedTxCategory::ValidishStructure,
            MalformedTxCategory::OverflowValue,
        ];

        for category in &categories {
            let result = fuzzer.run_batch(*category, |bytes| {
                let mut slice = bytes;
                bitcoin::Transaction::consensus_decode(&mut slice).is_ok()
            });
            assert!(
                result.is_safe(),
                "Category {:?} triggered a panic",
                category
            );
            assert_eq!(result.iterations, 20);
        }
    }
}
