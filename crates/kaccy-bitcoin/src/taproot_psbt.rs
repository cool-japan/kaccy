//! BIP 371 — Taproot fields for PSBT.
//!
//! Adds Taproot-specific key-value fields to Partially Signed Bitcoin
//! Transactions for key-path signing, script-path signing, and Taproot
//! BIP 32 derivation paths.
//!
//! The new fields defined by BIP 371 cover:
//!
//! | Field | Key type | Description |
//! |---|---|---|
//! | `PSBT_IN_TAP_KEY_SIG` | `0x13` | Schnorr key-path signature |
//! | `PSBT_IN_TAP_SCRIPT_SIG` | `0x14` | Schnorr script-path signature |
//! | `PSBT_IN_TAP_LEAF_SCRIPT` | `0x15` | Script + control block |
//! | `PSBT_IN_TAP_BIP32_DERIVATION` | `0x16` | x-only pubkey derivation |
//! | `PSBT_IN_TAP_INTERNAL_KEY` | `0x17` | x-only internal key |
//! | `PSBT_IN_TAP_MERKLE_ROOT` | `0x18` | Taproot Merkle root |
//! | `PSBT_OUT_TAP_INTERNAL_KEY` | `0x05` | x-only output internal key |
//! | `PSBT_OUT_TAP_TREE` | `0x06` | Tapscript tree |
//! | `PSBT_OUT_TAP_BIP32_DERIVATION` | `0x07` | x-only output key derivation |
//!
//! # References
//! - [BIP 371](https://github.com/bitcoin/bips/blob/master/bip-0371.mediawiki)
//! - [BIP 341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ──────────────────────────────────────────────────────────────────────────────
// Errors
// ──────────────────────────────────────────────────────────────────────────────

/// Errors specific to BIP 371 Taproot PSBT fields.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TaprootPsbtError {
    /// An invalid key path was supplied.
    #[error("invalid key path: {0}")]
    InvalidKeyPath(String),

    /// A leaf script is malformed.
    #[error("invalid leaf script: {0}")]
    InvalidLeafScript(String),

    /// A control block is malformed.
    #[error("invalid control block: {0}")]
    InvalidControlBlock(String),

    /// A Schnorr signature is malformed.
    #[error("invalid Taproot signature: {0}")]
    InvalidSignature(String),

    /// An x-only internal key is malformed.
    #[error("invalid internal key: {0}")]
    InvalidInternalKey(String),

    /// A Taproot Merkle root is malformed.
    #[error("invalid Merkle root: {0}")]
    InvalidMerkleRoot(String),

    /// A BIP 32 derivation path is malformed.
    #[error("invalid derivation path: {0}")]
    InvalidDerivationPath(String),

    /// An input index is out of range.
    #[error("input index {0} out of range")]
    InputIndexOutOfRange(usize),

    /// An output index is out of range.
    #[error("output index {0} out of range")]
    OutputIndexOutOfRange(usize),
}

// ──────────────────────────────────────────────────────────────────────────────
// Private helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Validate that `hex_str` encodes exactly `expected_bytes` bytes.
fn validate_hex_length(
    hex_str: &str,
    expected_bytes: usize,
    field: &str,
) -> Result<(), TaprootPsbtError> {
    let expected_chars = expected_bytes * 2;
    if hex_str.len() != expected_chars {
        return Err(TaprootPsbtError::InvalidInternalKey(format!(
            "{field}: expected {expected_bytes} bytes ({expected_chars} hex chars), got {} chars",
            hex_str.len()
        )));
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(TaprootPsbtError::InvalidInternalKey(format!(
            "{field}: contains non-hex characters"
        )));
    }
    Ok(())
}

/// Validate a hex string of exactly `expected_bytes` bytes, returning a
/// domain-specific error variant wrapped in the closure `make_err`.
fn validate_hex_with<E, F>(hex_str: &str, expected_bytes: usize, make_err: F) -> Result<(), E>
where
    F: FnOnce(String) -> E,
{
    let expected_chars = expected_bytes * 2;
    if hex_str.len() != expected_chars {
        return Err(make_err(format!(
            "expected {expected_bytes} bytes ({expected_chars} hex chars), got {} chars",
            hex_str.len()
        )));
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(make_err("contains non-hex characters".to_string()));
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// LeafVersion
// ──────────────────────────────────────────────────────────────────────────────

/// Tapscript leaf version as defined in BIP 341.
///
/// The only standardised version is `0xC0` (TapScript).  Future soft-forks
/// may introduce additional versions; those are represented by [`Future`].
///
/// [`Future`]: LeafVersion::Future
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LeafVersion {
    /// Standard Tapscript (`0xC0`).
    #[default]
    TapScript,
    /// A future or non-standard leaf version.
    Future(u8),
}

impl LeafVersion {
    /// Encode the leaf version as a single byte.
    pub fn to_byte(&self) -> u8 {
        match self {
            Self::TapScript => 0xC0,
            Self::Future(b) => *b,
        }
    }

    /// Decode a leaf version from a single byte.
    pub fn from_byte(b: u8) -> Self {
        if b == 0xC0 {
            Self::TapScript
        } else {
            Self::Future(b)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TapLeafScript
// ──────────────────────────────────────────────────────────────────────────────

/// A Tapscript leaf (`PSBT_IN_TAP_LEAF_SCRIPT`).
///
/// Stores the raw script bytes together with the control block that proves
/// membership in the Taproot commitment tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapLeafScript {
    /// Tapscript leaf version.
    pub leaf_version: LeafVersion,

    /// Raw script bytes.
    pub script: Vec<u8>,

    /// Control block bytes (Merkle proof).
    ///
    /// The control block is at least 33 bytes (1-byte version/parity + 32-byte
    /// internal key), followed by zero or more 32-byte Merkle proof elements.
    pub control_block: Vec<u8>,
}

impl TapLeafScript {
    /// Construct a new leaf script with an empty control block.
    pub fn new(script: Vec<u8>, version: LeafVersion) -> Self {
        Self {
            leaf_version: version,
            script,
            control_block: Vec::new(),
        }
    }

    /// Return the script as a lowercase hex string.
    pub fn script_hex(&self) -> String {
        self.script
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    }

    /// Return the control block as a lowercase hex string.
    pub fn control_block_hex(&self) -> String {
        self.control_block
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    }

    /// Number of 32-byte Merkle proof elements in the control block.
    ///
    /// The minimum well-formed control block is 33 bytes (version byte +
    /// 32-byte internal key).  Each additional 32 bytes is one proof element.
    /// Returns `0` if the control block is empty or shorter than 33 bytes.
    pub fn proof_len(&self) -> usize {
        if self.control_block.len() < 33 {
            return 0;
        }
        (self.control_block.len() - 33) / 32
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TapBip32Derivation
// ──────────────────────────────────────────────────────────────────────────────

/// BIP 32 derivation record for a Taproot x-only public key
/// (`PSBT_IN_TAP_BIP32_DERIVATION` / `PSBT_OUT_TAP_BIP32_DERIVATION`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapBip32Derivation {
    /// 32-byte x-only public key as lowercase hex (64 chars).
    pub x_only_pubkey: String,

    /// 4-byte master key fingerprint as lowercase hex (8 chars).
    pub master_fingerprint: String,

    /// BIP 32 derivation path, e.g. `"m/86'/0'/0'/0/0"`.
    pub derivation_path: String,

    /// Tapleaf hashes that this key signs for.  An empty vector indicates
    /// key-path spending.
    pub leaf_hashes: Vec<String>,
}

impl TapBip32Derivation {
    /// Construct a derivation record.
    pub fn new(x_only_pubkey: String, master_fingerprint: String, derivation_path: String) -> Self {
        Self {
            x_only_pubkey,
            master_fingerprint,
            derivation_path,
            leaf_hashes: Vec::new(),
        }
    }

    /// Returns `true` when this key is used for key-path spending (no leaf
    /// hashes specified).
    pub fn is_key_path(&self) -> bool {
        self.leaf_hashes.is_empty()
    }

    /// Validate the derivation record.
    pub fn validate(&self) -> Result<(), TaprootPsbtError> {
        // x-only pubkey: 32 bytes = 64 hex chars.
        validate_hex_with(&self.x_only_pubkey, 32, |msg| {
            TaprootPsbtError::InvalidKeyPath(format!("x_only_pubkey: {msg}"))
        })?;

        // Master fingerprint: 4 bytes = 8 hex chars.
        validate_hex_with(&self.master_fingerprint, 4, |msg| {
            TaprootPsbtError::InvalidDerivationPath(format!("master_fingerprint: {msg}"))
        })?;

        if self.derivation_path.is_empty() {
            return Err(TaprootPsbtError::InvalidDerivationPath(
                "derivation path is empty".to_string(),
            ));
        }

        // Validate each leaf hash is 32 bytes.
        for hash in &self.leaf_hashes {
            validate_hex_with(hash, 32, |msg| {
                TaprootPsbtError::InvalidKeyPath(format!("leaf_hash: {msg}"))
            })?;
        }

        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TaprootInputFields
// ──────────────────────────────────────────────────────────────────────────────

/// BIP 371 Taproot fields for a single PSBT input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaprootInputFields {
    /// Schnorr signature for key-path spending (`PSBT_IN_TAP_KEY_SIG`).
    ///
    /// Must be 64 bytes (without sighash type) or 65 bytes (with sighash type).
    pub tap_key_sig: Option<String>,

    /// Script-path Schnorr signatures: `(control_block_hex, sig_hex)` pairs
    /// (`PSBT_IN_TAP_SCRIPT_SIG`).
    pub tap_script_sigs: Vec<(String, String)>,

    /// Tapscript leaves in the spend path (`PSBT_IN_TAP_LEAF_SCRIPT`).
    pub tap_leaf_scripts: Vec<TapLeafScript>,

    /// BIP 32 derivation paths for Taproot keys
    /// (`PSBT_IN_TAP_BIP32_DERIVATION`).
    pub tap_bip32_derivations: Vec<TapBip32Derivation>,

    /// x-only internal key (`PSBT_IN_TAP_INTERNAL_KEY`).
    pub tap_internal_key: Option<String>,

    /// Taproot Merkle root (`PSBT_IN_TAP_MERKLE_ROOT`).
    ///
    /// `None` indicates a key-path-only output (no script tree).
    pub tap_merkle_root: Option<String>,
}

impl TaprootInputFields {
    /// Construct an empty set of Taproot input fields.
    pub fn new() -> Self {
        Self {
            tap_key_sig: None,
            tap_script_sigs: Vec::new(),
            tap_leaf_scripts: Vec::new(),
            tap_bip32_derivations: Vec::new(),
            tap_internal_key: None,
            tap_merkle_root: None,
        }
    }

    /// Attach a key-path Schnorr signature.
    ///
    /// The signature must be exactly 64 bytes (without explicit sighash type)
    /// or 65 bytes (with an appended sighash type byte).
    pub fn with_key_sig(mut self, sig: String) -> Result<Self, TaprootPsbtError> {
        let is_valid = sig.len() == 128 /* 64 bytes */ || sig.len() == 130 /* 65 bytes */;
        if !is_valid || !sig.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TaprootPsbtError::InvalidSignature(format!(
                "tap_key_sig must be 64 or 65 bytes (128 or 130 hex chars), got {} chars",
                sig.len()
            )));
        }
        self.tap_key_sig = Some(sig);
        Ok(self)
    }

    /// Attach the x-only internal key.
    ///
    /// Must be exactly 32 bytes (64 hex chars).
    pub fn with_internal_key(mut self, key: String) -> Result<Self, TaprootPsbtError> {
        validate_hex_length(&key, 32, "tap_internal_key").map_err(|_| {
            TaprootPsbtError::InvalidInternalKey(format!(
                "tap_internal_key must be 32 bytes (64 hex chars), got {} chars",
                key.len()
            ))
        })?;
        self.tap_internal_key = Some(key);
        Ok(self)
    }

    /// Attach the Taproot Merkle root.
    ///
    /// Must be exactly 32 bytes (64 hex chars).
    pub fn with_merkle_root(mut self, root: String) -> Result<Self, TaprootPsbtError> {
        validate_hex_with(&root, 32, TaprootPsbtError::InvalidMerkleRoot)?;
        self.tap_merkle_root = Some(root);
        Ok(self)
    }

    /// Append a Tapscript leaf.
    pub fn add_leaf_script(&mut self, leaf: TapLeafScript) {
        self.tap_leaf_scripts.push(leaf);
    }

    /// Append a BIP 32 derivation record.
    pub fn add_bip32_derivation(&mut self, deriv: TapBip32Derivation) {
        self.tap_bip32_derivations.push(deriv);
    }

    /// Returns `true` when a key-path Schnorr signature is present.
    pub fn is_key_path_signed(&self) -> bool {
        self.tap_key_sig.is_some()
    }

    /// Returns `true` when at least one script-path signature is present.
    pub fn is_script_path_signed(&self) -> bool {
        !self.tap_script_sigs.is_empty()
    }

    /// Returns `true` when the input is finalised via either key-path or
    /// script-path signing.
    pub fn is_finalized(&self) -> bool {
        self.is_key_path_signed() || self.is_script_path_signed()
    }

    /// Validate all Taproot fields.
    pub fn validate(&self) -> Result<(), TaprootPsbtError> {
        if let Some(ref sig) = self.tap_key_sig {
            let valid = (sig.len() == 128 || sig.len() == 130)
                && sig.chars().all(|c| c.is_ascii_hexdigit());
            if !valid {
                return Err(TaprootPsbtError::InvalidSignature(format!(
                    "tap_key_sig: expected 64 or 65 bytes, got {} hex chars",
                    sig.len()
                )));
            }
        }

        if let Some(ref key) = self.tap_internal_key {
            validate_hex_length(key, 32, "tap_internal_key").map_err(|_| {
                TaprootPsbtError::InvalidInternalKey(format!(
                    "tap_internal_key must be 32 bytes (64 hex chars), got {} chars",
                    key.len()
                ))
            })?;
        }

        if let Some(ref root) = self.tap_merkle_root {
            validate_hex_with(root, 32, TaprootPsbtError::InvalidMerkleRoot)?;
        }

        for (cb, sig) in &self.tap_script_sigs {
            // Control block: minimum 33 bytes, multiples of 32 thereafter.
            if cb.len() < 66 || cb.len() % 2 != 0 || !cb.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(TaprootPsbtError::InvalidControlBlock(format!(
                    "script sig control block is malformed (len {})",
                    cb.len()
                )));
            }
            let sig_valid = (sig.len() == 128 || sig.len() == 130)
                && sig.chars().all(|c| c.is_ascii_hexdigit());
            if !sig_valid {
                return Err(TaprootPsbtError::InvalidSignature(format!(
                    "script-path sig must be 64 or 65 bytes, got {} hex chars",
                    sig.len()
                )));
            }
        }

        for deriv in &self.tap_bip32_derivations {
            deriv.validate()?;
        }

        Ok(())
    }
}

impl Default for TaprootInputFields {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TaprootOutputFields
// ──────────────────────────────────────────────────────────────────────────────

/// BIP 371 Taproot fields for a single PSBT output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaprootOutputFields {
    /// x-only internal key (`PSBT_OUT_TAP_INTERNAL_KEY`).
    pub tap_internal_key: Option<String>,

    /// Tapscript tree (`PSBT_OUT_TAP_TREE`).
    pub tap_tree: Vec<TapLeafScript>,

    /// BIP 32 derivation paths for x-only output keys
    /// (`PSBT_OUT_TAP_BIP32_DERIVATION`).
    pub tap_bip32_derivations: Vec<TapBip32Derivation>,
}

impl TaprootOutputFields {
    /// Construct an empty set of Taproot output fields.
    pub fn new() -> Self {
        Self {
            tap_internal_key: None,
            tap_tree: Vec::new(),
            tap_bip32_derivations: Vec::new(),
        }
    }

    /// Attach the x-only internal key.
    ///
    /// Must be exactly 32 bytes (64 hex chars).
    pub fn with_internal_key(mut self, key: String) -> Result<Self, TaprootPsbtError> {
        validate_hex_with(&key, 32, |msg| {
            TaprootPsbtError::InvalidInternalKey(format!("tap_internal_key: {msg}"))
        })?;
        self.tap_internal_key = Some(key);
        Ok(self)
    }

    /// Append a Tapscript tree leaf.
    pub fn add_tree_leaf(&mut self, leaf: TapLeafScript) {
        self.tap_tree.push(leaf);
    }

    /// Returns `true` when an internal key is set, indicating this is a Taproot
    /// output.
    pub fn is_taproot_output(&self) -> bool {
        self.tap_internal_key.is_some()
    }

    /// Validate all Taproot output fields.
    pub fn validate(&self) -> Result<(), TaprootPsbtError> {
        if let Some(ref key) = self.tap_internal_key {
            validate_hex_with(key, 32, |msg| {
                TaprootPsbtError::InvalidInternalKey(format!("tap_internal_key: {msg}"))
            })?;
        }

        for deriv in &self.tap_bip32_derivations {
            deriv.validate()?;
        }

        Ok(())
    }
}

impl Default for TaprootOutputFields {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TaprootPsbt
// ──────────────────────────────────────────────────────────────────────────────

/// A PSBT with BIP 371 Taproot fields attached.
///
/// The base PSBT data is stored as a hex-encoded byte string.  The
/// `input_fields` and `output_fields` vectors are indexed to match the inputs
/// and outputs of the base PSBT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaprootPsbt {
    /// The serialised base PSBT, hex-encoded.
    pub base_psbt_hex: String,

    /// Taproot fields for each input, indexed to match the base PSBT inputs.
    pub input_fields: Vec<TaprootInputFields>,

    /// Taproot fields for each output, indexed to match the base PSBT outputs.
    pub output_fields: Vec<TaprootOutputFields>,
}

impl TaprootPsbt {
    /// Wrap an existing hex-encoded PSBT.
    pub fn new(psbt_hex: String) -> Self {
        Self {
            base_psbt_hex: psbt_hex,
            input_fields: Vec::new(),
            output_fields: Vec::new(),
        }
    }

    /// Number of input Taproot field records currently attached.
    pub fn input_count(&self) -> usize {
        self.input_fields.len()
    }

    /// Number of output Taproot field records currently attached.
    pub fn output_count(&self) -> usize {
        self.output_fields.len()
    }

    /// Returns `true` when every attached input field set is finalised.
    ///
    /// Returns `false` when no input fields are attached.
    pub fn is_complete(&self) -> bool {
        if self.input_fields.is_empty() {
            return false;
        }
        self.input_fields.iter().all(|f| f.is_finalized())
    }

    /// Return a reference to the Taproot fields for input `index`, or `None`
    /// if the index is out of range.
    pub fn get_input_fields(&self, index: usize) -> Option<&TaprootInputFields> {
        self.input_fields.get(index)
    }

    /// Return a reference to the Taproot fields for output `index`, or `None`
    /// if the index is out of range.
    pub fn get_output_fields(&self, index: usize) -> Option<&TaprootOutputFields> {
        self.output_fields.get(index)
    }

    /// Attach or replace the Taproot fields for input at `index`.
    ///
    /// If `index` is beyond the current length of `input_fields`, the vector
    /// is extended with default (empty) records to fill the gap.
    pub fn add_input_taproot_fields(
        &mut self,
        index: usize,
        fields: TaprootInputFields,
    ) -> Result<(), TaprootPsbtError> {
        // Extend with defaults if needed.
        while self.input_fields.len() <= index {
            self.input_fields.push(TaprootInputFields::new());
        }
        self.input_fields[index] = fields;
        Ok(())
    }

    /// Attach or replace the Taproot fields for output at `index`.
    ///
    /// If `index` is beyond the current length of `output_fields`, the vector
    /// is extended with default (empty) records to fill the gap.
    pub fn add_output_taproot_fields(
        &mut self,
        index: usize,
        fields: TaprootOutputFields,
    ) -> Result<(), TaprootPsbtError> {
        while self.output_fields.len() <= index {
            self.output_fields.push(TaprootOutputFields::new());
        }
        self.output_fields[index] = fields;
        Ok(())
    }

    /// Validate all attached input and output Taproot fields.
    pub fn validate_all(&self) -> Result<(), TaprootPsbtError> {
        for fields in &self.input_fields {
            fields.validate()?;
        }
        for fields in &self.output_fields {
            fields.validate()?;
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TaprootPsbtBuilder
// ──────────────────────────────────────────────────────────────────────────────

/// Fluent builder for [`TaprootPsbt`].
///
/// # Example
///
/// ```rust
/// use kaccy_bitcoin::taproot_psbt::{
///     TaprootPsbtBuilder, TaprootInputFields, TaprootOutputFields,
/// };
///
/// let psbt = TaprootPsbtBuilder::new("deadbeef".to_string())
///     .add_input_fields(TaprootInputFields::new())
///     .add_output_fields(TaprootOutputFields::new())
///     .build();
///
/// assert_eq!(psbt.input_count(), 1);
/// assert_eq!(psbt.output_count(), 1);
/// ```
#[derive(Debug)]
pub struct TaprootPsbtBuilder {
    psbt_hex: String,
    input_fields: Vec<TaprootInputFields>,
    output_fields: Vec<TaprootOutputFields>,
}

impl TaprootPsbtBuilder {
    /// Create a new builder wrapping the given hex-encoded PSBT.
    pub fn new(psbt_hex: String) -> Self {
        Self {
            psbt_hex,
            input_fields: Vec::new(),
            output_fields: Vec::new(),
        }
    }

    /// Append input Taproot fields.
    #[must_use]
    pub fn add_input_fields(mut self, fields: TaprootInputFields) -> Self {
        self.input_fields.push(fields);
        self
    }

    /// Append output Taproot fields.
    #[must_use]
    pub fn add_output_fields(mut self, fields: TaprootOutputFields) -> Self {
        self.output_fields.push(fields);
        self
    }

    /// Consume the builder and produce a [`TaprootPsbt`].
    pub fn build(self) -> TaprootPsbt {
        TaprootPsbt {
            base_psbt_hex: self.psbt_hex,
            input_fields: self.input_fields,
            output_fields: self.output_fields,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 32-byte x-only pubkey hex (64 chars).
    fn xonly_key() -> String {
        "b2c9c8b6a5d3e1f0b2c9c8b6a5d3e1f0b2c9c8b6a5d3e1f0b2c9c8b6a5d3e1f0".to_string()
    }

    // 64-byte Schnorr signature (128 hex chars).
    fn schnorr_sig_64() -> String {
        "aa".repeat(64)
    }

    // 65-byte Schnorr signature (130 hex chars).
    fn schnorr_sig_65() -> String {
        format!("{}01", "bb".repeat(64))
    }

    // Minimal control block (33 bytes = 66 hex chars).
    fn minimal_control_block() -> String {
        "c0".to_string() + &"ab".repeat(32)
    }

    #[test]
    fn test_leaf_version_tapscript() {
        let v = LeafVersion::TapScript;
        assert_eq!(v.to_byte(), 0xC0);
        assert_eq!(LeafVersion::from_byte(0xC0), LeafVersion::TapScript);
    }

    #[test]
    fn test_leaf_version_future() {
        let v = LeafVersion::Future(0xC2);
        assert_eq!(v.to_byte(), 0xC2);
        assert_eq!(LeafVersion::from_byte(0xC2), LeafVersion::Future(0xC2));
    }

    #[test]
    fn test_tap_leaf_script_new() {
        let script = vec![0x51, 0x20]; // OP_1 OP_32
        let leaf = TapLeafScript::new(script.clone(), LeafVersion::TapScript);
        assert_eq!(leaf.leaf_version, LeafVersion::TapScript);
        assert_eq!(leaf.script, script);
        assert!(leaf.control_block.is_empty());
        assert_eq!(leaf.script_hex(), "5120");
    }

    #[test]
    fn test_tap_leaf_script_proof_len() {
        let mut leaf = TapLeafScript::new(vec![], LeafVersion::TapScript);

        // Empty control block → 0.
        assert_eq!(leaf.proof_len(), 0);

        // Control block with exactly 33 bytes → 0 proof elements.
        leaf.control_block = vec![0xc0; 33];
        assert_eq!(leaf.proof_len(), 0);

        // 33 + 32 = 65 bytes → 1 proof element.
        leaf.control_block = vec![0xc0; 65];
        assert_eq!(leaf.proof_len(), 1);

        // 33 + 64 = 97 bytes → 2 proof elements.
        leaf.control_block = vec![0xc0; 97];
        assert_eq!(leaf.proof_len(), 2);
    }

    #[test]
    fn test_taproot_input_fields_default() {
        let fields = TaprootInputFields::default();
        assert!(fields.tap_key_sig.is_none());
        assert!(fields.tap_internal_key.is_none());
        assert!(fields.tap_merkle_root.is_none());
        assert!(fields.tap_script_sigs.is_empty());
        assert!(fields.tap_leaf_scripts.is_empty());
        assert!(fields.tap_bip32_derivations.is_empty());
        assert!(!fields.is_finalized());
    }

    #[test]
    fn test_taproot_input_key_path_signed() {
        let fields = TaprootInputFields::new()
            .with_key_sig(schnorr_sig_64())
            .expect("64-byte sig should be accepted");

        assert!(fields.is_key_path_signed());
        assert!(!fields.is_script_path_signed());
        assert!(fields.is_finalized());
        assert!(fields.validate().is_ok());
    }

    #[test]
    fn test_taproot_input_key_sig_65_bytes() {
        let fields = TaprootInputFields::new()
            .with_key_sig(schnorr_sig_65())
            .expect("65-byte sig should be accepted");

        assert!(fields.is_key_path_signed());
        assert!(fields.validate().is_ok());
    }

    #[test]
    fn test_taproot_input_key_sig_invalid_length() {
        // 63 bytes = 126 hex chars — invalid.
        let bad_sig = "cc".repeat(63);
        let result = TaprootInputFields::new().with_key_sig(bad_sig);
        assert!(
            matches!(result, Err(TaprootPsbtError::InvalidSignature(_))),
            "expected InvalidSignature"
        );
    }

    #[test]
    fn test_taproot_input_internal_key_valid() {
        let fields = TaprootInputFields::new()
            .with_internal_key(xonly_key())
            .expect("valid 32-byte key");

        assert!(fields.tap_internal_key.is_some());
        assert!(fields.validate().is_ok());
    }

    #[test]
    fn test_taproot_input_internal_key_invalid() {
        let short_key = "aabb".to_string(); // only 2 bytes
        let result = TaprootInputFields::new().with_internal_key(short_key);
        assert!(
            matches!(result, Err(TaprootPsbtError::InvalidInternalKey(_))),
            "expected InvalidInternalKey"
        );
    }

    #[test]
    fn test_taproot_output_fields() {
        let fields = TaprootOutputFields::new()
            .with_internal_key(xonly_key())
            .expect("valid key");

        assert!(fields.is_taproot_output());
        assert!(fields.validate().is_ok());
    }

    #[test]
    fn test_taproot_output_fields_default_not_taproot() {
        let fields = TaprootOutputFields::default();
        assert!(!fields.is_taproot_output());
        assert!(fields.validate().is_ok());
    }

    #[test]
    fn test_taproot_psbt_new() {
        let psbt = TaprootPsbt::new("deadbeef".to_string());
        assert_eq!(psbt.base_psbt_hex, "deadbeef");
        assert_eq!(psbt.input_count(), 0);
        assert_eq!(psbt.output_count(), 0);
        assert!(!psbt.is_complete());
    }

    #[test]
    fn test_taproot_psbt_add_input_fields() {
        let mut psbt = TaprootPsbt::new("deadbeef".to_string());

        let fields = TaprootInputFields::new()
            .with_key_sig(schnorr_sig_64())
            .expect("valid sig");

        psbt.add_input_taproot_fields(0, fields).unwrap();
        assert_eq!(psbt.input_count(), 1);

        // is_complete() should be true since the one input is finalised.
        assert!(psbt.is_complete());

        // get_input_fields
        let retrieved = psbt.get_input_fields(0);
        assert!(retrieved.is_some());
        assert!(retrieved.unwrap().is_key_path_signed());

        // Out-of-range returns None.
        assert!(psbt.get_input_fields(99).is_none());
    }

    #[test]
    fn test_taproot_psbt_add_output_fields_sparse() {
        let mut psbt = TaprootPsbt::new("cafebabe".to_string());

        // Insert at index 2 — should extend the vector.
        let fields = TaprootOutputFields::new()
            .with_internal_key(xonly_key())
            .expect("valid key");

        psbt.add_output_taproot_fields(2, fields).unwrap();
        assert_eq!(psbt.output_count(), 3); // indices 0, 1 are defaults
        assert!(psbt.get_output_fields(2).unwrap().is_taproot_output());
        assert!(!psbt.get_output_fields(0).unwrap().is_taproot_output());
    }

    #[test]
    fn test_bip32_derivation() {
        let deriv = TapBip32Derivation::new(
            xonly_key(),
            "deadbeef".to_string(), // 4-byte fingerprint
            "m/86'/0'/0'/0/0".to_string(),
        );

        assert!(deriv.is_key_path());
        assert!(deriv.validate().is_ok());
    }

    #[test]
    fn test_bip32_derivation_invalid_fingerprint() {
        let deriv = TapBip32Derivation::new(
            xonly_key(),
            "dead".to_string(), // only 2 bytes — should fail
            "m/86'/0'/0'/0/0".to_string(),
        );
        let result = deriv.validate();
        assert!(
            matches!(result, Err(TaprootPsbtError::InvalidDerivationPath(_))),
            "expected InvalidDerivationPath for bad fingerprint, got {result:?}"
        );
    }

    #[test]
    fn test_taproot_psbt_builder() {
        let input_fields = TaprootInputFields::new()
            .with_key_sig(schnorr_sig_64())
            .expect("valid sig");

        let output_fields = TaprootOutputFields::new()
            .with_internal_key(xonly_key())
            .expect("valid key");

        let psbt = TaprootPsbtBuilder::new("aabbccdd".to_string())
            .add_input_fields(input_fields)
            .add_output_fields(output_fields)
            .build();

        assert_eq!(psbt.base_psbt_hex, "aabbccdd");
        assert_eq!(psbt.input_count(), 1);
        assert_eq!(psbt.output_count(), 1);
        assert!(psbt.is_complete());
        assert!(psbt.validate_all().is_ok());
    }

    #[test]
    fn test_validate_all_with_script_path_sigs() {
        let mut input_fields = TaprootInputFields::new();
        // Add a script-path sig with a minimal control block and valid sig.
        input_fields
            .tap_script_sigs
            .push((minimal_control_block(), schnorr_sig_64()));

        let psbt = TaprootPsbtBuilder::new("ff".to_string())
            .add_input_fields(input_fields)
            .build();

        assert!(psbt.is_complete());
        assert!(psbt.validate_all().is_ok());
    }

    #[test]
    fn test_merkle_root_validation() {
        // Exactly 64 hex chars = 32 bytes (32 * "ab").
        let valid_root = "ab".repeat(32);
        let fields = TaprootInputFields::new()
            .with_merkle_root(valid_root)
            .expect("valid 32-byte merkle root");

        assert!(fields.tap_merkle_root.is_some());
        assert!(fields.validate().is_ok());

        // Short merkle root should fail.
        let result = TaprootInputFields::new().with_merkle_root("deadbeef".to_string()); // only 4 bytes
        assert!(
            matches!(result, Err(TaprootPsbtError::InvalidMerkleRoot(_))),
            "expected InvalidMerkleRoot for short value"
        );
    }

    #[test]
    fn test_tap_leaf_script_add_and_retrieve() {
        let mut input_fields = TaprootInputFields::new();

        let leaf1 = TapLeafScript::new(vec![0x51], LeafVersion::TapScript);
        let leaf2 = TapLeafScript::new(vec![0x52], LeafVersion::Future(0xC2));

        input_fields.add_leaf_script(leaf1);
        input_fields.add_leaf_script(leaf2);

        assert_eq!(input_fields.tap_leaf_scripts.len(), 2);
        assert_eq!(input_fields.tap_leaf_scripts[0].script_hex(), "51");
        assert_eq!(
            input_fields.tap_leaf_scripts[1].leaf_version,
            LeafVersion::Future(0xC2)
        );
    }
}
