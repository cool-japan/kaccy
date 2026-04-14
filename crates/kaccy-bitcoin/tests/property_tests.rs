//! Property-based tests using proptest for bitcoin utilities.
//!
//! Verifies invariants that should hold for all inputs within a domain.

use kaccy_bitcoin::{AddressConverter, Bech32mCodec, SchnorrKeyPair};
use proptest::prelude::*;

proptest! {
    /// Normalization must be idempotent: applying it twice gives the same result as once.
    #[test]
    fn test_address_normalize_idempotent(s in "[a-zA-Z0-9]{10,50}") {
        let once = AddressConverter::normalize(&s);
        let twice = AddressConverter::normalize(&once);
        prop_assert_eq!(&once, &twice);
    }

    /// Normalize must always produce lowercase output.
    #[test]
    fn test_address_normalize_always_lowercase(s in "[a-zA-Z0-9]{5,40}") {
        let normalized = AddressConverter::normalize(&s);
        prop_assert_eq!(&normalized, &normalized.to_lowercase());
    }

    /// Normalize must strip leading and trailing whitespace.
    #[test]
    fn test_address_normalize_trims_whitespace(
        prefix in "  *",
        body in "[a-zA-Z0-9]{5,20}",
        suffix in "  *",
    ) {
        let padded = format!("{}{}{}", prefix, body, suffix);
        let normalized = AddressConverter::normalize(&padded);
        prop_assert!(!normalized.starts_with(' '));
        prop_assert!(!normalized.ends_with(' '));
    }

    /// Non-segwit inputs should never pass is_bech32m or is_bech32.
    #[test]
    fn test_random_strings_not_valid_segwit(s in "[a-z0-9]{20,50}") {
        // Unless by extreme coincidence the random string is a valid bech32(m)
        // address, these checks should be false. We test with strings that
        // deliberately lack the valid prefixes.
        let no_prefix = format!("xyz{}", s);
        prop_assert!(!Bech32mCodec::is_bech32m(&no_prefix));
        prop_assert!(!Bech32mCodec::is_bech32(&no_prefix));
    }
}

/// Generate fresh keypairs: public key must always be 32 bytes.
#[test]
fn test_schnorr_keypair_pubkey_length() {
    for _ in 0..20 {
        let kp = SchnorrKeyPair::generate().expect("generate keypair");
        assert_eq!(kp.public_key().len(), 32);
        assert_eq!(kp.public_key_bytes().len(), 32);
    }
}

/// Legacy P2PKH address must not be classified as bech32 or bech32m.
#[test]
fn test_bech32m_is_not_legacy() {
    let legacy_p2pkh = "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2";
    assert!(!Bech32mCodec::is_bech32m(legacy_p2pkh));
    assert!(!Bech32mCodec::is_bech32(legacy_p2pkh));
}

/// BIP 341 Taproot test vector must be identified as bech32m.
#[test]
fn test_bech32m_taproot_address() {
    let taproot = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";
    assert!(
        Bech32mCodec::is_bech32m(taproot),
        "known Taproot address must be bech32m"
    );
    assert!(
        !Bech32mCodec::is_bech32(taproot),
        "Taproot must not be bech32 (v0)"
    );
}

/// P2WPKH address must be classified as bech32 (v0), not bech32m.
#[test]
fn test_bech32_segwit_v0_address() {
    let p2wpkh = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    assert!(
        Bech32mCodec::is_bech32(p2wpkh),
        "P2WPKH address must be bech32"
    );
    assert!(
        !Bech32mCodec::is_bech32m(p2wpkh),
        "P2WPKH address must not be bech32m"
    );
}

// ─── Fuzz-based property tests ────────────────────────────────────────────────

use kaccy_bitcoin::fuzz_testing::{FuzzConfig, MalformedTxCategory, TransactionFuzzer};

proptest! {
    /// Parsing arbitrary byte sequences must never cause a panic.
    ///
    /// Errors (structured `Err`) are the expected outcome for random bytes;
    /// only silent panics would indicate a safety regression.
    #[test]
    fn test_fuzz_random_bytes_no_panic(bytes in proptest::collection::vec(0u8.., 0..1000)) {
        use bitcoin::consensus::Decodable;
        let mut slice: &[u8] = &bytes;
        // We only check that this does not panic.
        let _result = bitcoin::Transaction::consensus_decode(&mut slice);
    }
}

#[test]
fn test_transaction_fuzzer_safety() {
    let config = FuzzConfig {
        max_iterations: 100,
        ..Default::default()
    };
    let fuzzer = TransactionFuzzer::new(config);

    use bitcoin::consensus::Decodable;
    let result = fuzzer.run_batch(MalformedTxCategory::RandomNoise, |bytes| {
        let mut slice = bytes;
        bitcoin::Transaction::consensus_decode(&mut slice).is_ok()
    });
    assert!(result.is_safe(), "No panics should occur during fuzzing");
}

#[test]
fn test_truncated_tx_fuzz_safety() {
    let config = FuzzConfig {
        max_iterations: 50,
        ..Default::default()
    };
    let fuzzer = TransactionFuzzer::new(config);

    use bitcoin::consensus::Decodable;
    let result = fuzzer.run_batch(MalformedTxCategory::TruncatedHeader, |bytes| {
        let mut slice = bytes;
        bitcoin::Transaction::consensus_decode(&mut slice).is_ok()
    });
    assert!(result.is_safe());
}

#[test]
fn test_malformed_tx_fuzz_all_categories() {
    use bitcoin::consensus::Decodable;

    let config = FuzzConfig {
        max_iterations: 30,
        ..Default::default()
    };
    let fuzzer = TransactionFuzzer::new(config);

    let categories = [
        MalformedTxCategory::TruncatedHeader,
        MalformedTxCategory::RandomNoise,
        MalformedTxCategory::ValidishStructure,
        MalformedTxCategory::OverflowValue,
    ];

    for category in &categories {
        let result = fuzzer.run_batch(*category, |bytes| {
            let mut slice = bytes;
            bitcoin::Transaction::consensus_decode(&mut slice).is_ok()
        });
        assert!(result.is_safe(), "Category {:?} caused a panic", category);
    }
}
