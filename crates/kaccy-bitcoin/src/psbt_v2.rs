//! BIP 370 — PSBT Version 2 support.
//!
//! PSBT version 2 allows inputs and outputs to be added independently
//! without a pre-constructed unsigned transaction, enabling more flexible
//! multi-party transaction construction protocols.
//!
//! Unlike PSBT v0 (BIP 174), version 2 stores the transaction data in
//! per-input and per-output key-value fields rather than embedding a full
//! unsigned transaction in the global map. This enables incremental
//! construction: new inputs and outputs can be added by different parties
//! before the PSBT is sealed.
//!
//! # References
//! - [BIP 370](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// `hex` is a workspace dependency used for decoding pubkey bytes in `PsbtV2Output::p2wpkh`.
use hex;

// ──────────────────────────────────────────────────────────────────────────────
// Errors
// ──────────────────────────────────────────────────────────────────────────────

/// Errors specific to BIP 370 PSBT version 2 operations.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum PsbtV2Error {
    /// The supplied PSBT is not version 2.
    #[error("invalid PSBT version {0}; expected 2")]
    InvalidVersion(u32),

    /// A required BIP 370 field is absent.
    #[error("missing required field: {0}")]
    MissingRequiredField(String),

    /// Locktime values are incompatible (e.g. mixing height and time).
    #[error("invalid locktime: {0}")]
    InvalidLocktime(String),

    /// An invalid sequence number was supplied.
    #[error("invalid sequence number: {0}")]
    InvalidSequence(u32),

    /// The requested input index is out of range.
    #[error("input index {0} out of range")]
    InputIndexOutOfRange(usize),

    /// The requested output index is out of range.
    #[error("output index {0} out of range")]
    OutputIndexOutOfRange(usize),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// An attempt was made to modify a sealed (non-modifiable) PSBT.
    #[error("modifiability violation: {0}")]
    ModifiabilityViolation(String),
}

// ──────────────────────────────────────────────────────────────────────────────
// TxModifiable
// ──────────────────────────────────────────────────────────────────────────────

/// Bit-flag field encoding which parts of a PSBT v2 may still be modified.
///
/// | Bit | Meaning                                         |
/// |-----|-------------------------------------------------|
/// | 0   | Inputs modifiable                               |
/// | 1   | Outputs modifiable                              |
/// | 2   | Has a SIGHASH_SINGLE input                      |
///
/// When the value is `0x00` the PSBT is considered *sealed* and no further
/// inputs or outputs may be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxModifiable(pub u8);

impl TxModifiable {
    /// Both inputs and outputs are modifiable.
    pub const INPUTS_OUTPUTS_MODIFIABLE: Self = Self(0x03);
    /// Only inputs may be added.
    pub const INPUTS_MODIFIABLE: Self = Self(0x01);
    /// Only outputs may be added.
    pub const OUTPUTS_MODIFIABLE: Self = Self(0x02);
    /// At least one input uses SIGHASH_SINGLE.
    pub const HAS_SIGHASH_SINGLE: Self = Self(0x04);
    /// Sealed — no further modification allowed.
    pub const NONE: Self = Self(0x00);

    /// Returns `true` if inputs may still be added.
    #[inline]
    pub fn inputs_modifiable(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Returns `true` if outputs may still be added.
    #[inline]
    pub fn outputs_modifiable(&self) -> bool {
        self.0 & 0x02 != 0
    }

    /// Returns `true` if at least one input uses SIGHASH_SINGLE.
    #[inline]
    pub fn has_sighash_single(&self) -> bool {
        self.0 & 0x04 != 0
    }

    /// Returns `true` when the PSBT is sealed (no bits set).
    #[inline]
    pub fn is_sealed(&self) -> bool {
        self.0 == 0
    }
}

impl Default for TxModifiable {
    /// Default to sealed (no modifications allowed).
    fn default() -> Self {
        Self::NONE
    }
}

impl std::ops::BitOr for TxModifiable {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for TxModifiable {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Private helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Validate that `hex_str` is a valid lower-case hex string encoding exactly
/// `expected_bytes` bytes.  Returns an error whose message names `field`.
fn validate_hex_length(
    hex_str: &str,
    expected_bytes: usize,
    field: &str,
) -> Result<(), PsbtV2Error> {
    let expected_chars = expected_bytes * 2;
    if hex_str.len() != expected_chars {
        return Err(PsbtV2Error::MissingRequiredField(format!(
            "{field}: expected {expected_bytes} bytes ({expected_chars} hex chars), got {} chars",
            hex_str.len()
        )));
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(PsbtV2Error::MissingRequiredField(format!(
            "{field}: contains non-hex characters"
        )));
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// PsbtV2Input
// ──────────────────────────────────────────────────────────────────────────────

/// A single input in a BIP 370 PSBT version 2.
///
/// In v2 the previous outpoint is stored per-input rather than in an unsigned
/// transaction embedded in the global map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtV2Input {
    /// 32-byte transaction ID of the UTXO being spent, encoded as lowercase hex.
    pub previous_txid: String,

    /// Index of the output in `previous_txid` that is being spent.
    pub output_index: u32,

    /// Optional sequence number override (`PSBT_IN_SEQUENCE`).
    pub sequence: Option<u32>,

    /// Optional time-based locktime constraint (`PSBT_IN_REQUIRED_TIME_LOCKTIME`).
    /// When set, the transaction locktime must be ≥ this value and ≥ 500 000 000
    /// (UNIX timestamp range).
    pub required_time_locktime: Option<u32>,

    /// Optional height-based locktime constraint (`PSBT_IN_REQUIRED_HEIGHT_LOCKTIME`).
    /// When set, the transaction locktime must be ≥ this value and < 500 000 000
    /// (block height range).
    pub required_height_locktime: Option<u32>,

    /// Serialized UTXO being spent, as hex (witness UTXO field).
    pub witness_utxo: Option<String>,

    /// Finalised scriptSig for this input, as hex.
    pub final_script_sig: Option<String>,

    /// Finalised witness stack for this input.  Each element is hex-encoded.
    pub final_script_witness: Option<Vec<String>>,

    /// Sighash type for this input.
    pub sighash_type: Option<u32>,

    /// Partial signatures collected so far: `(pubkey_hex, sig_hex)`.
    pub partial_sigs: Vec<(String, String)>,
}

impl PsbtV2Input {
    /// Construct a new input referencing the given outpoint.
    ///
    /// `previous_txid` must be a 64-character lowercase hex string (32 bytes).
    pub fn new(previous_txid: String, output_index: u32) -> Self {
        Self {
            previous_txid,
            output_index,
            sequence: None,
            required_time_locktime: None,
            required_height_locktime: None,
            witness_utxo: None,
            final_script_sig: None,
            final_script_witness: None,
            sighash_type: None,
            partial_sigs: Vec::new(),
        }
    }

    /// Set the sequence number for this input.
    #[must_use]
    pub fn with_sequence(mut self, seq: u32) -> Self {
        self.sequence = Some(seq);
        self
    }

    /// Set the required time-based locktime for this input.
    ///
    /// Per BIP 370, the value must be ≥ 500 000 000 to be in the UNIX-timestamp
    /// range.  The constructor stores the value verbatim; [`validate`] checks
    /// the range.
    ///
    /// [`validate`]: PsbtV2Input::validate
    #[must_use]
    pub fn with_time_locktime(mut self, locktime: u32) -> Self {
        self.required_time_locktime = Some(locktime);
        self
    }

    /// Set the required block-height-based locktime for this input.
    #[must_use]
    pub fn with_height_locktime(mut self, locktime: u32) -> Self {
        self.required_height_locktime = Some(locktime);
        self
    }

    /// Returns `true` when this input has been finalised (has a final
    /// scriptSig or a final witness).
    pub fn is_finalized(&self) -> bool {
        self.final_script_sig.is_some() || self.final_script_witness.is_some()
    }

    /// Validate the input fields for BIP 370 compliance.
    ///
    /// Checks:
    /// - `previous_txid` is 64 hex chars (32 bytes).
    /// - Time-based and height-based locktimes are not both set simultaneously.
    /// - Time locktime is in the UNIX-timestamp range (≥ 500 000 000).
    /// - Height locktime is in the block-height range (< 500 000 000).
    pub fn validate(&self) -> Result<(), PsbtV2Error> {
        validate_hex_length(&self.previous_txid, 32, "previous_txid")?;

        if self.required_time_locktime.is_some() && self.required_height_locktime.is_some() {
            return Err(PsbtV2Error::InvalidLocktime(
                "cannot set both time-based and height-based locktimes on the same input"
                    .to_string(),
            ));
        }

        if let Some(t) = self.required_time_locktime {
            if t < 500_000_000 {
                return Err(PsbtV2Error::InvalidLocktime(format!(
                    "required_time_locktime {t} is below the UNIX-timestamp range (500_000_000)"
                )));
            }
        }

        if let Some(h) = self.required_height_locktime {
            if h >= 500_000_000 {
                return Err(PsbtV2Error::InvalidLocktime(format!(
                    "required_height_locktime {h} is not in the block-height range (< 500_000_000)"
                )));
            }
        }

        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PsbtV2Output
// ──────────────────────────────────────────────────────────────────────────────

/// A single output in a BIP 370 PSBT version 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtV2Output {
    /// Output value in satoshis.
    pub amount: u64,

    /// Output script (scriptPubKey) as lowercase hex.
    pub script_pubkey: String,

    /// Redeem script for P2SH outputs (hex).
    pub redeem_script: Option<String>,

    /// Witness script for P2WSH outputs (hex).
    pub witness_script: Option<String>,

    /// BIP 32 derivation paths associated with this output:
    /// `(compressed_pubkey_hex, derivation_path)`.
    pub bip32_derivations: Vec<(String, String)>,
}

impl PsbtV2Output {
    /// Construct a new output with the given amount and scriptPubKey.
    pub fn new(amount: u64, script_pubkey: String) -> Self {
        Self {
            amount,
            script_pubkey,
            redeem_script: None,
            witness_script: None,
            bip32_derivations: Vec::new(),
        }
    }

    /// Convenience constructor for a P2WPKH output.
    ///
    /// Computes a P2WPKH scriptPubKey (`OP_0 OP_PUSH20 <HASH160(pubkey)>`)
    /// from a 33-byte compressed public key encoded as hex.
    ///
    /// HASH160 is computed correctly as RIPEMD160(SHA256(pubkey_bytes)), which
    /// is the Bitcoin standard required for all on-chain addresses and scripts.
    ///
    /// # Panics
    /// Panics if `pubkey_hex` is not valid lowercase hex (i.e. contains
    /// non-hexadecimal characters or has an odd length).  Callers must
    /// supply a well-formed compressed public-key hex string (66 chars = 33 bytes).
    pub fn p2wpkh(amount: u64, pubkey_hex: &str) -> Self {
        // Decode the hex-encoded public key to raw bytes.
        let pubkey_bytes = hex::decode(pubkey_hex).expect("pubkey_hex must be valid hex");

        // Compute Bitcoin HASH160 = RIPEMD160(SHA256(pubkey_bytes)).
        let hash20 = hash160_of_bytes(&pubkey_bytes);

        // Encode the 20-byte witness program as lowercase hex.
        let keyhash_hex: String = hash20.iter().map(|b| format!("{b:02x}")).collect();

        // P2WPKH scriptPubKey: OP_0 OP_PUSHBYTES_20 <20-byte-hash>
        let script_pubkey = format!("0014{keyhash_hex}");
        Self::new(amount, script_pubkey)
    }

    /// Validate the output fields.
    pub fn validate(&self) -> Result<(), PsbtV2Error> {
        if self.script_pubkey.is_empty() {
            return Err(PsbtV2Error::MissingRequiredField(
                "script_pubkey".to_string(),
            ));
        }
        if self.script_pubkey.len() % 2 != 0 {
            return Err(PsbtV2Error::MissingRequiredField(
                "script_pubkey: odd hex length".to_string(),
            ));
        }
        Ok(())
    }
}

/// Compute Bitcoin HASH160 = RIPEMD160(SHA256(data)) and return the 20-byte result.
///
/// This is the exact hash function used for all P2PKH and P2WPKH addresses in Bitcoin.
/// Using any substitute (e.g. truncated SHA-256) produces an irrecoverable wrong witness
/// program and makes the output permanently unspendable.
fn hash160_of_bytes(data: &[u8]) -> [u8; 20] {
    use bitcoin::hashes::{Hash, hash160};
    hash160::Hash::hash(data).to_byte_array()
}

// ──────────────────────────────────────────────────────────────────────────────
// PsbtV2Summary
// ──────────────────────────────────────────────────────────────────────────────

/// A compact, human-readable summary of a [`PsbtV2`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtV2Summary {
    /// PSBT version (always 2).
    pub version: u32,
    /// Number of inputs.
    pub input_count: usize,
    /// Number of outputs.
    pub output_count: usize,
    /// Sum of all output values in satoshis.
    pub total_output_value: u64,
    /// Whether all inputs have been finalised.
    pub is_complete: bool,
    /// Whether the PSBT has been sealed.
    pub is_sealed: bool,
    /// Effective locktime for the transaction.
    pub effective_locktime: u32,
}

// ──────────────────────────────────────────────────────────────────────────────
// PsbtV2
// ──────────────────────────────────────────────────────────────────────────────

/// A BIP 370 PSBT version 2.
///
/// Unlike v0, the transaction structure (version, locktime, input outpoints,
/// output scripts and amounts) is stored in per-input/output fields rather
/// than in a pre-built unsigned transaction embedded in the global map.
///
/// # Workflow
///
/// 1. Create with [`PsbtV2::new`] or [`PsbtV2Builder`].
/// 2. Add inputs with [`add_input`] and outputs with [`add_output`] while
///    the PSBT is still modifiable.
/// 3. Call [`seal`] when construction is complete.
/// 4. Parties attach partial signatures; check completeness with
///    [`is_complete`].
///
/// [`add_input`]: PsbtV2::add_input
/// [`add_output`]: PsbtV2::add_output
/// [`seal`]: PsbtV2::seal
/// [`is_complete`]: PsbtV2::is_complete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtV2 {
    /// PSBT format version — always `2` for this type.
    pub version: u32,

    /// Bitcoin transaction version (1 or 2).
    pub tx_version: u32,

    /// Fallback locktime used when no input specifies a locktime constraint.
    pub fallback_locktime: Option<u32>,

    /// Which parts of this PSBT are still open for modification.
    pub modifiable: TxModifiable,

    /// Input records.
    pub inputs: Vec<PsbtV2Input>,

    /// Output records.
    pub outputs: Vec<PsbtV2Output>,

    /// Unrecognised global key-value pairs preserved verbatim.
    pub unknown_globals: HashMap<String, String>,
}

impl PsbtV2 {
    /// Create a new, empty, fully-modifiable BIP 370 PSBT.
    ///
    /// The `modifiable` field is set to `INPUTS_MODIFIABLE | OUTPUTS_MODIFIABLE`
    /// so that both inputs and outputs can be added before the PSBT is sealed.
    pub fn new(tx_version: u32) -> Self {
        Self {
            version: 2,
            tx_version,
            fallback_locktime: None,
            modifiable: TxModifiable::INPUTS_MODIFIABLE | TxModifiable::OUTPUTS_MODIFIABLE,
            inputs: Vec::new(),
            outputs: Vec::new(),
            unknown_globals: HashMap::new(),
        }
    }

    /// Attempt to add an input.
    ///
    /// Returns [`PsbtV2Error::ModifiabilityViolation`] if the `INPUTS_MODIFIABLE`
    /// bit is not set.
    pub fn add_input(&mut self, input: PsbtV2Input) -> Result<(), PsbtV2Error> {
        if !self.modifiable.inputs_modifiable() {
            return Err(PsbtV2Error::ModifiabilityViolation(
                "PSBT inputs are not modifiable".to_string(),
            ));
        }
        self.inputs.push(input);
        Ok(())
    }

    /// Attempt to add an output.
    ///
    /// Returns [`PsbtV2Error::ModifiabilityViolation`] if the `OUTPUTS_MODIFIABLE`
    /// bit is not set.
    pub fn add_output(&mut self, output: PsbtV2Output) -> Result<(), PsbtV2Error> {
        if !self.modifiable.outputs_modifiable() {
            return Err(PsbtV2Error::ModifiabilityViolation(
                "PSBT outputs are not modifiable".to_string(),
            ));
        }
        self.outputs.push(output);
        Ok(())
    }

    /// Seal the PSBT: clear the `INPUTS_MODIFIABLE` and `OUTPUTS_MODIFIABLE` bits.
    ///
    /// After sealing, [`add_input`] and [`add_output`] will return errors.
    ///
    /// [`add_input`]: PsbtV2::add_input
    /// [`add_output`]: PsbtV2::add_output
    pub fn seal(&mut self) {
        self.modifiable = TxModifiable::NONE;
    }

    /// Returns `true` when every input has been finalised.
    pub fn is_complete(&self) -> bool {
        if self.inputs.is_empty() {
            return false;
        }
        self.inputs.iter().all(|i| i.is_finalized())
    }

    /// Number of inputs.
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Number of outputs.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    /// Sum of all output amounts in satoshis.
    pub fn total_output_value(&self) -> u64 {
        self.outputs.iter().map(|o| o.amount).sum()
    }

    /// Validate the entire PSBT for BIP 370 compliance.
    ///
    /// - Checks that `version == 2`.
    /// - Validates every input.
    /// - Validates every output.
    /// - Checks that time-based and height-based locktime constraints are not
    ///   mixed across inputs (BIP 370 §Consensus).
    pub fn validate(&self) -> Result<(), PsbtV2Error> {
        if self.version != 2 {
            return Err(PsbtV2Error::InvalidVersion(self.version));
        }

        for input in &self.inputs {
            input.validate()?;
        }

        for output in &self.outputs {
            output.validate()?;
        }

        // BIP 370: all inputs must agree on whether they use time-based or
        // height-based locktimes; mixing is not allowed.
        let has_time = self
            .inputs
            .iter()
            .any(|i| i.required_time_locktime.is_some());
        let has_height = self
            .inputs
            .iter()
            .any(|i| i.required_height_locktime.is_some());

        if has_time && has_height {
            return Err(PsbtV2Error::InvalidLocktime(
                "inputs mix time-based and height-based locktime requirements".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate the effective locktime per BIP 370.
    ///
    /// - If any input has `required_time_locktime`, the effective locktime is
    ///   the maximum of those values.
    /// - Otherwise if any input has `required_height_locktime`, the effective
    ///   locktime is the maximum of those values.
    /// - Otherwise, `fallback_locktime` is used (defaulting to `0`).
    pub fn effective_locktime(&self) -> u32 {
        let max_time: Option<u32> = self
            .inputs
            .iter()
            .filter_map(|i| i.required_time_locktime)
            .max();

        if let Some(t) = max_time {
            return t;
        }

        let max_height: Option<u32> = self
            .inputs
            .iter()
            .filter_map(|i| i.required_height_locktime)
            .max();

        if let Some(h) = max_height {
            return h;
        }

        self.fallback_locktime.unwrap_or(0)
    }

    /// Build a compact summary of this PSBT.
    pub fn to_summary(&self) -> PsbtV2Summary {
        PsbtV2Summary {
            version: self.version,
            input_count: self.input_count(),
            output_count: self.output_count(),
            total_output_value: self.total_output_value(),
            is_complete: self.is_complete(),
            is_sealed: self.modifiable.is_sealed(),
            effective_locktime: self.effective_locktime(),
        }
    }

    /// Serialise the PSBT as a key-value map suitable for JSON export.
    ///
    /// The map keys use BIP 370 field names where applicable.
    pub fn serialize_to_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert(
            "PSBT_GLOBAL_VERSION".to_string(),
            serde_json::Value::from(self.version),
        );
        map.insert(
            "PSBT_GLOBAL_TX_VERSION".to_string(),
            serde_json::Value::from(self.tx_version),
        );
        if let Some(lt) = self.fallback_locktime {
            map.insert(
                "PSBT_GLOBAL_FALLBACK_LOCKTIME".to_string(),
                serde_json::Value::from(lt),
            );
        }
        map.insert(
            "PSBT_GLOBAL_INPUT_COUNT".to_string(),
            serde_json::Value::from(self.inputs.len()),
        );
        map.insert(
            "PSBT_GLOBAL_OUTPUT_COUNT".to_string(),
            serde_json::Value::from(self.outputs.len()),
        );
        map.insert(
            "PSBT_GLOBAL_TX_MODIFIABLE".to_string(),
            serde_json::Value::from(self.modifiable.0),
        );
        map.insert(
            "inputs".to_string(),
            serde_json::to_value(&self.inputs).unwrap_or(serde_json::Value::Null),
        );
        map.insert(
            "outputs".to_string(),
            serde_json::to_value(&self.outputs).unwrap_or(serde_json::Value::Null),
        );
        map
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PsbtV2Builder
// ──────────────────────────────────────────────────────────────────────────────

/// Fluent builder for [`PsbtV2`].
///
/// # Example
///
/// ```rust
/// use kaccy_bitcoin::psbt_v2::{PsbtV2Builder, PsbtV2Input, PsbtV2Output, TxModifiable};
///
/// let txid = "a".repeat(64);
/// let psbt = PsbtV2Builder::new()
///     .tx_version(2)
///     .fallback_locktime(0)
///     .modifiable(TxModifiable::INPUTS_MODIFIABLE | TxModifiable::OUTPUTS_MODIFIABLE)
///     .add_input(PsbtV2Input::new(txid, 0))
///     .add_output(PsbtV2Output::new(100_000, "0014aabbccdd".to_string()))
///     .build()
///     .expect("build should succeed");
///
/// assert_eq!(psbt.version, 2);
/// assert_eq!(psbt.input_count(), 1);
/// ```
#[derive(Debug)]
pub struct PsbtV2Builder {
    tx_version: u32,
    fallback_locktime: Option<u32>,
    modifiable: TxModifiable,
    inputs: Vec<PsbtV2Input>,
    outputs: Vec<PsbtV2Output>,
}

impl Default for PsbtV2Builder {
    /// Delegates to [`PsbtV2Builder::new`], ensuring `Default::default()`
    /// produces the same state as explicit construction (tx_version 2,
    /// fully modifiable).
    fn default() -> Self {
        Self::new()
    }
}

impl PsbtV2Builder {
    /// Create a new builder with default values (tx_version 2, fully modifiable).
    pub fn new() -> Self {
        Self {
            tx_version: 2,
            fallback_locktime: None,
            modifiable: TxModifiable::INPUTS_MODIFIABLE | TxModifiable::OUTPUTS_MODIFIABLE,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Set the transaction version.
    #[must_use]
    pub fn tx_version(mut self, v: u32) -> Self {
        self.tx_version = v;
        self
    }

    /// Set the fallback locktime.
    #[must_use]
    pub fn fallback_locktime(mut self, lt: u32) -> Self {
        self.fallback_locktime = Some(lt);
        self
    }

    /// Override the `TxModifiable` flags.
    #[must_use]
    pub fn modifiable(mut self, m: TxModifiable) -> Self {
        self.modifiable = m;
        self
    }

    /// Append an input.
    ///
    /// The builder bypasses the modifiability check; [`PsbtV2::add_input`]
    /// enforces it at runtime after construction.
    #[must_use]
    pub fn add_input(mut self, input: PsbtV2Input) -> Self {
        self.inputs.push(input);
        self
    }

    /// Append an output.
    #[must_use]
    pub fn add_output(mut self, output: PsbtV2Output) -> Self {
        self.outputs.push(output);
        self
    }

    /// Consume the builder and produce a [`PsbtV2`], or return an error if
    /// validation fails.
    pub fn build(self) -> Result<PsbtV2, PsbtV2Error> {
        let psbt = PsbtV2 {
            version: 2,
            tx_version: self.tx_version,
            fallback_locktime: self.fallback_locktime,
            modifiable: self.modifiable,
            inputs: self.inputs,
            outputs: self.outputs,
            unknown_globals: HashMap::new(),
        };
        psbt.validate()?;
        Ok(psbt)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: a syntactically valid 32-byte txid (64 lowercase hex chars).
    fn dummy_txid() -> String {
        "a".repeat(64)
    }

    // Helper: create a minimal valid input.
    fn minimal_input() -> PsbtV2Input {
        PsbtV2Input::new(dummy_txid(), 0)
    }

    // Helper: create a minimal valid output.
    fn minimal_output() -> PsbtV2Output {
        PsbtV2Output::new(
            100_000,
            "0014aabbccddeeff00112233445566778899aabb".to_string(),
        )
    }

    #[test]
    fn test_tx_modifiable_flags() {
        let m = TxModifiable::INPUTS_MODIFIABLE | TxModifiable::OUTPUTS_MODIFIABLE;
        assert!(m.inputs_modifiable());
        assert!(m.outputs_modifiable());
        assert!(!m.has_sighash_single());
        assert!(!m.is_sealed());
    }

    #[test]
    fn test_tx_modifiable_sealed() {
        let m = TxModifiable::NONE;
        assert!(!m.inputs_modifiable());
        assert!(!m.outputs_modifiable());
        assert!(m.is_sealed());
    }

    #[test]
    fn test_tx_modifiable_bitor() {
        let m = TxModifiable::INPUTS_MODIFIABLE | TxModifiable::HAS_SIGHASH_SINGLE;
        assert!(m.inputs_modifiable());
        assert!(!m.outputs_modifiable());
        assert!(m.has_sighash_single());
    }

    #[test]
    fn test_psbt_v2_new() {
        let psbt = PsbtV2::new(2);
        assert_eq!(psbt.version, 2);
        assert_eq!(psbt.tx_version, 2);
        assert!(psbt.modifiable.inputs_modifiable());
        assert!(psbt.modifiable.outputs_modifiable());
        assert_eq!(psbt.input_count(), 0);
        assert_eq!(psbt.output_count(), 0);
    }

    #[test]
    fn test_add_input_sealed_fails() {
        let mut psbt = PsbtV2::new(2);
        psbt.seal();
        let result = psbt.add_input(minimal_input());
        assert!(matches!(
            result,
            Err(PsbtV2Error::ModifiabilityViolation(_))
        ));
    }

    #[test]
    fn test_add_output_sealed_fails() {
        let mut psbt = PsbtV2::new(2);
        psbt.seal();
        let result = psbt.add_output(minimal_output());
        assert!(matches!(
            result,
            Err(PsbtV2Error::ModifiabilityViolation(_))
        ));
    }

    #[test]
    fn test_is_complete_no_inputs() {
        let psbt = PsbtV2::new(2);
        // No inputs → not complete.
        assert!(!psbt.is_complete());
    }

    #[test]
    fn test_is_complete_with_finalized_input() {
        let mut psbt = PsbtV2::new(2);
        let mut input = minimal_input();
        input.final_script_sig = Some("deadbeef".to_string());
        psbt.add_input(input).unwrap();
        assert!(psbt.is_complete());
    }

    #[test]
    fn test_total_output_value() {
        let mut psbt = PsbtV2::new(2);
        psbt.add_output(PsbtV2Output::new(50_000, "0014aa".to_string()))
            .unwrap();
        psbt.add_output(PsbtV2Output::new(75_000, "0014bb".to_string()))
            .unwrap();
        assert_eq!(psbt.total_output_value(), 125_000);
    }

    #[test]
    fn test_builder_basic() {
        let psbt = PsbtV2Builder::new()
            .tx_version(2)
            .fallback_locktime(0)
            .add_input(minimal_input())
            .add_output(minimal_output())
            .build()
            .expect("builder should produce a valid PSBT");

        assert_eq!(psbt.version, 2);
        assert_eq!(psbt.input_count(), 1);
        assert_eq!(psbt.output_count(), 1);
        assert_eq!(psbt.fallback_locktime, Some(0));
    }

    #[test]
    fn test_effective_locktime_from_inputs() {
        let mut psbt = PsbtV2Builder::new()
            .fallback_locktime(100)
            .add_input(PsbtV2Input::new(dummy_txid(), 0).with_height_locktime(800_000))
            .add_input(PsbtV2Input::new(dummy_txid(), 1).with_height_locktime(850_000))
            .build()
            .expect("valid PSBT");

        // Effective locktime should be max of height locktimes.
        assert_eq!(psbt.effective_locktime(), 850_000);

        // After removing all inputs the fallback should be used.
        psbt.inputs.clear();
        assert_eq!(psbt.effective_locktime(), 100);
    }

    #[test]
    fn test_effective_locktime_time_based() {
        let psbt = PsbtV2Builder::new()
            .add_input(PsbtV2Input::new(dummy_txid(), 0).with_time_locktime(1_700_000_000))
            .add_input(PsbtV2Input::new(dummy_txid(), 1).with_time_locktime(1_800_000_000))
            .build()
            .expect("valid PSBT");

        assert_eq!(psbt.effective_locktime(), 1_800_000_000);
    }

    #[test]
    fn test_input_validation_missing_txid() {
        let short_txid = "aabb".to_string(); // only 2 bytes, not 32
        let input = PsbtV2Input::new(short_txid, 0);
        let result = input.validate();
        assert!(
            matches!(result, Err(PsbtV2Error::MissingRequiredField(_))),
            "expected MissingRequiredField, got {result:?}"
        );
    }

    #[test]
    fn test_input_validation_mixed_locktimes() {
        let input = PsbtV2Input::new(dummy_txid(), 0)
            .with_time_locktime(1_700_000_000)
            .with_height_locktime(800_000);
        let result = input.validate();
        assert!(
            matches!(result, Err(PsbtV2Error::InvalidLocktime(_))),
            "expected InvalidLocktime, got {result:?}"
        );
    }

    #[test]
    fn test_psbt_v2_summary() {
        let mut psbt = PsbtV2::new(2);
        let mut input = minimal_input();
        input.final_script_sig = Some("cafebabe".to_string());
        psbt.add_input(input).unwrap();
        psbt.add_output(minimal_output()).unwrap();
        psbt.seal();

        let summary = psbt.to_summary();
        assert_eq!(summary.version, 2);
        assert_eq!(summary.input_count, 1);
        assert_eq!(summary.output_count, 1);
        assert_eq!(summary.total_output_value, 100_000);
        assert!(summary.is_complete);
        assert!(summary.is_sealed);
    }

    #[test]
    fn test_serialize_to_map() {
        let psbt = PsbtV2::new(2);
        let map = psbt.serialize_to_map();
        assert!(map.contains_key("PSBT_GLOBAL_VERSION"));
        assert!(map.contains_key("PSBT_GLOBAL_TX_VERSION"));
        assert!(map.contains_key("PSBT_GLOBAL_INPUT_COUNT"));
        assert!(map.contains_key("PSBT_GLOBAL_OUTPUT_COUNT"));
        assert!(map.contains_key("PSBT_GLOBAL_TX_MODIFIABLE"));
    }

    #[test]
    fn test_p2wpkh_output_convenience() {
        let pubkey_hex = "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
        let output = PsbtV2Output::p2wpkh(50_000, pubkey_hex);
        assert_eq!(output.amount, 50_000);
        // P2WPKH script must start with "0014" (OP_0 OP_PUSHBYTES_20).
        assert!(
            output.script_pubkey.starts_with("0014"),
            "P2WPKH script should start with 0014"
        );
        // Total length: 4 (OP_0 OP_PUSH20 prefix hex) + 40 (20 bytes of witness program hex).
        assert_eq!(output.script_pubkey.len(), 44);
    }

    /// Known-answer test: verifies that `PsbtV2Output::p2wpkh` produces the
    /// EXACT Bitcoin-standard HASH160 = RIPEMD160(SHA256(pubkey)) for the
    /// well-known compressed public key used throughout BIP test vectors.
    ///
    /// The expected HASH160 is computed independently by the `bitcoin` crate's
    /// `hash160::Hash::hash` so that the test proves both:
    ///   1. The implementation applies real HASH160 (not truncated SHA-256).
    ///   2. The resulting P2WPKH scriptPubKey has the correct `OP_0 <20-byte-hash>` form.
    #[test]
    fn test_p2wpkh_hash160_known_answer() {
        use bitcoin::hashes::{Hash as _, hash160};

        // BIP test-vector compressed public key (SEC1 compressed, 33 bytes).
        let pubkey_hex = "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";

        // Decode and compute the expected HASH160 directly with the bitcoin crate.
        let pubkey_bytes = hex::decode(pubkey_hex).expect("valid hex");
        let expected_hash = hash160::Hash::hash(&pubkey_bytes);
        let expected_hash_hex: String = expected_hash
            .to_byte_array()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        // The expected scriptPubKey: OP_0 OP_PUSHBYTES_20 <hash160>.
        let expected_script = format!("0014{expected_hash_hex}");

        // Build the output using the fixed implementation.
        let output = PsbtV2Output::p2wpkh(100_000, pubkey_hex);

        // Structural checks.
        assert_eq!(output.amount, 100_000);
        assert!(
            output.script_pubkey.starts_with("0014"),
            "P2WPKH scriptPubKey must begin with OP_0 OP_PUSHBYTES_20 (hex: 0014)"
        );
        assert_eq!(
            output.script_pubkey.len(),
            44,
            "P2WPKH scriptPubKey must be exactly 22 bytes (4 prefix + 40 hash hex chars)"
        );

        // Exact known-answer check: the 20-byte witness program must equal the
        // bitcoin-crate-computed HASH160, proving we use RIPEMD160(SHA256(·))
        // and not any truncated or approximate substitute.
        assert_eq!(
            output.script_pubkey, expected_script,
            "P2WPKH scriptPubKey mismatch: got {}, want {}",
            output.script_pubkey, expected_script
        );

        // Cross-check: confirm the embedded 20-byte hash is 20 bytes of real HASH160.
        let embedded_hash_hex = &output.script_pubkey[4..]; // strip "0014"
        assert_eq!(
            embedded_hash_hex, expected_hash_hex,
            "Embedded witness program does not match RIPEMD160(SHA256(pubkey))"
        );
    }

    #[test]
    fn test_validate_mixed_locktime_across_inputs_fails() {
        // Build bypasses the check; validate() should catch it.
        let mut psbt = PsbtV2 {
            version: 2,
            tx_version: 2,
            fallback_locktime: None,
            modifiable: TxModifiable::NONE,
            inputs: vec![
                PsbtV2Input::new(dummy_txid(), 0).with_time_locktime(1_700_000_000),
                PsbtV2Input::new(dummy_txid(), 1).with_height_locktime(800_000),
            ],
            outputs: Vec::new(),
            unknown_globals: HashMap::new(),
        };
        // Individual input validation: first input is valid alone.
        assert!(psbt.inputs[0].validate().is_ok());
        assert!(psbt.inputs[1].validate().is_ok());
        // But cross-input validation in PsbtV2::validate() should fail.
        let result = psbt.validate();
        assert!(
            matches!(result, Err(PsbtV2Error::InvalidLocktime(_))),
            "expected cross-input locktime error, got {result:?}"
        );
        // Fix: remove the height-based one.
        psbt.inputs[1].required_height_locktime = None;
        assert!(psbt.validate().is_ok());
    }
}
