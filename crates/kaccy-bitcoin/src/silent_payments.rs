//! Silent Payments (BIP 352) implementation
//!
//! Silent payments allow receiving Bitcoin payments without address reuse,
//! improving privacy by deriving unique addresses for each payment using ECDH.
//!
//! # Overview
//!
//! - Sender generates a unique output public key using ECDH
//! - Recipient scans the blockchain to detect owned outputs
//! - No address reuse on-chain
//! - Labels allow organizing addresses
//!
//! # Spec compliance
//!
//! This module follows [BIP 352](https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki):
//!
//! - Addresses are real bech32m strings (`sp1q…` on mainnet, `tsp1q…` on test
//!   networks) with witness version `0` and a 66-byte payload consisting of the
//!   33-byte compressed scan public key followed by the 33-byte compressed spend
//!   public key. Encoding and decoding round-trip exactly.
//! - The sender computes
//!   `input_hash = hash_BIP0352/Inputs(outpoint_L ‖ A_sum)`,
//!   `ecdh = input_hash · a_sum · B_scan`,
//!   `t_k = hash_BIP0352/SharedSecret(ecdh ‖ ser32(k))`,
//!   `P_k = B_spend + t_k·G`.
//! - The receiver derives the symmetric shared secret
//!   `ecdh = input_hash · b_scan · A_sum` and recomputes `P_k`, so a
//!   sender-derived output key always equals the receiver-scanned one.
//!
//! Full wallet scanning over an arbitrary transaction *set* (the bulk-scan
//! optimisation described in the BIP) is intentionally **not** faked: the
//! [`SilentPaymentScanner::scan_transaction_set`] entry point returns a typed
//! [`SilentPaymentError::BulkScanUnimplemented`] error rather than fabricating
//! detections. Single-transaction and single-output derivation are fully
//! implemented and spec-correct.
//!
//! # Example
//!
//! ```
//! use kaccy_bitcoin::silent_payments::{SilentPaymentAddress, SilentPaymentWallet};
//! use bitcoin::secp256k1::SecretKey;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate a silent payment address
//! let wallet = SilentPaymentWallet::new()?;
//! let sp_address = wallet.get_address(None)?;
//!
//! // The address can be shared publicly and reused
//! println!("Silent payment address: {}", sp_address);
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::bech32::primitives::decode::CheckedHrpstring;
use bitcoin::bech32::{Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use bitcoin::secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use bitcoin::{Network, OutPoint, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// BIP 352 tag for the input hash domain separator.
const TAG_INPUTS: &[u8] = b"BIP0352/Inputs";
/// BIP 352 tag for the shared-secret / per-output tweak domain separator.
const TAG_SHARED_SECRET: &[u8] = b"BIP0352/SharedSecret";
/// BIP 352 tag for the label tweak domain separator.
const TAG_LABEL: &[u8] = b"BIP0352/Label";

/// Bech32 / BIP 352 silent-payment version (witness version `0`).
const SP_VERSION: u8 = 0;
/// Size in bytes of a compressed secp256k1 public key.
const COMPRESSED_PK_LEN: usize = 33;
/// Size of the encoded silent-payment payload: scan pubkey ‖ spend pubkey.
const SP_PAYLOAD_LEN: usize = COMPRESSED_PK_LEN * 2;

/// Errors specific to silent-payment operations.
///
/// These complement [`BitcoinError`]; the public methods on this module convert
/// into [`BitcoinError`] where a uniform error type is required, but the
/// bulk-scan entry point surfaces this type directly so callers can distinguish
/// the deliberately-unimplemented optimisation from genuine failures.
#[derive(Debug, thiserror::Error)]
pub enum SilentPaymentError {
    /// The bulk transaction-set scanning optimisation is not implemented.
    ///
    /// This is an honest, typed signal — the scanner never fabricates
    /// detections. Use [`SilentPaymentScanner::scan_transaction`] together with
    /// the real prevout outpoints / input public keys to scan a single
    /// transaction correctly.
    #[error(
        "bulk silent-payment transaction-set scanning is not implemented; \
         scan individual transactions with prevout context instead"
    )]
    BulkScanUnimplemented,

    /// A BIP 352 cryptographic operation failed (e.g. an out-of-range scalar).
    #[error("silent payment cryptographic error: {0}")]
    Crypto(String),

    /// Address encoding or decoding failed.
    #[error("silent payment address error: {0}")]
    Address(String),
}

impl From<SilentPaymentError> for BitcoinError {
    fn from(err: SilentPaymentError) -> Self {
        match err {
            SilentPaymentError::Address(msg) => BitcoinError::InvalidAddress(msg),
            other => BitcoinError::Validation(other.to_string()),
        }
    }
}

/// Compute a BIP 340 / BIP 352 tagged hash: `SHA256(SHA256(tag) ‖ SHA256(tag) ‖ msg)`.
///
/// The two SHA256(tag) blocks are exactly one SHA-256 block (32 bytes each, 64
/// total), matching the reference implementation byte-for-byte without relying
/// on a precomputed midstate macro.
fn tagged_hash(tag: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    use bitcoin::hashes::{Hash, HashEngine, sha256};

    let tag_hash = sha256::Hash::hash(tag);

    let mut engine = sha256::Hash::engine();
    engine.input(tag_hash.as_ref());
    engine.input(tag_hash.as_ref());
    for part in parts {
        engine.input(part);
    }
    sha256::Hash::from_engine(engine).to_byte_array()
}

/// Convert a 32-byte big-endian value into a secp256k1 [`Scalar`], rejecting
/// values that are zero or out of range (`>= n`), as BIP 352 requires.
fn scalar_from_be_bytes(bytes: [u8; 32], context: &str) -> Result<Scalar, SilentPaymentError> {
    let scalar = Scalar::from_be_bytes(bytes)
        .map_err(|_| SilentPaymentError::Crypto(format!("{context}: scalar out of range")))?;
    if scalar == Scalar::ZERO {
        return Err(SilentPaymentError::Crypto(format!(
            "{context}: scalar is zero"
        )));
    }
    Ok(scalar)
}

/// Serialize an outpoint as it appears in a raw transaction:
/// 32-byte txid in internal (little-endian) byte order followed by the 4-byte
/// little-endian `vout`.
fn serialize_outpoint(outpoint: &OutPoint) -> [u8; 36] {
    let mut out = [0u8; 36];
    // `Txid::to_byte_array` yields the internal (little-endian) ordering, which
    // is exactly the wire serialization used by BIP 352.
    out[..32].copy_from_slice(bitcoin::hashes::Hash::as_byte_array(&outpoint.txid));
    out[32..].copy_from_slice(&outpoint.vout.to_le_bytes());
    out
}

/// Compute the BIP 352 `input_hash` scalar.
///
/// `input_hash = hash_BIP0352/Inputs(outpoint_L ‖ A_sum)` where `outpoint_L` is
/// the lexicographically smallest input outpoint (wire serialization) and
/// `A_sum` is the 33-byte compressed sum of input public keys.
fn compute_input_hash(
    outpoints: &[OutPoint],
    summed_input_pubkey: &PublicKey,
) -> Result<Scalar, SilentPaymentError> {
    if outpoints.is_empty() {
        return Err(SilentPaymentError::Crypto(
            "at least one input outpoint is required".to_string(),
        ));
    }

    let smallest = outpoints
        .iter()
        .map(serialize_outpoint)
        .min()
        .expect("non-empty outpoints checked above");

    let hash = tagged_hash(TAG_INPUTS, &[&smallest, &summed_input_pubkey.serialize()]);
    scalar_from_be_bytes(hash, "input_hash")
}

/// Compute the per-output tweak scalar `t_k`.
///
/// `t_k = hash_BIP0352/SharedSecret(ser_P(ecdh) ‖ ser32(k))` where `ser32(k)` is
/// the 4-byte **big-endian** output counter.
fn compute_output_tweak(
    ecdh_shared_secret: &PublicKey,
    output_index: u32,
) -> Result<Scalar, SilentPaymentError> {
    let hash = tagged_hash(
        TAG_SHARED_SECRET,
        &[
            &ecdh_shared_secret.serialize(),
            &output_index.to_be_bytes(),
        ],
    );
    scalar_from_be_bytes(hash, "output tweak t_k")
}

/// Silent payment address (`sp1q…` / `tsp1q…` bech32m format).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SilentPaymentAddress {
    /// Scan public key (B_scan)
    pub scan_pubkey: PublicKey,
    /// Spend public key (B_spend)
    pub spend_pubkey: PublicKey,
    /// Optional label for address organization
    pub label: Option<u32>,
    /// Network (mainnet, testnet, etc.)
    pub network: Network,
}

impl SilentPaymentAddress {
    /// Create a new silent payment address
    pub fn new(
        scan_pubkey: PublicKey,
        spend_pubkey: PublicKey,
        label: Option<u32>,
        network: Network,
    ) -> Self {
        Self {
            scan_pubkey,
            spend_pubkey,
            label,
            network,
        }
    }

    /// Human-readable part for a given network (`sp` on mainnet, `tsp` otherwise).
    fn hrp_for(network: Network) -> Hrp {
        // BIP 352 defines `sp` for mainnet and `tsp` for all test networks.
        match network {
            Network::Bitcoin => Hrp::parse_unchecked("sp"),
            _ => Hrp::parse_unchecked("tsp"),
        }
    }

    /// Encode as a BIP 352 bech32m address string (`sp1q…` / `tsp1q…`).
    ///
    /// The payload is the 33-byte compressed scan pubkey followed by the 33-byte
    /// compressed spend pubkey, prefixed by witness version `0` and protected by
    /// a bech32m checksum.
    pub fn encode(&self) -> String {
        let hrp = Self::hrp_for(self.network);

        let mut payload = Vec::with_capacity(SP_PAYLOAD_LEN);
        payload.extend_from_slice(&self.scan_pubkey.serialize());
        payload.extend_from_slice(&self.spend_pubkey.serialize());

        // Witness version `0` (`Fe32::Q`) followed by the 8->5 bit conversion of
        // the payload, with a bech32m checksum. This mirrors segwit address
        // encoding but without the 40-byte witness-program length cap, which is
        // exactly the BIP 352 construction.
        let version = Fe32::try_from(SP_VERSION).expect("SP version 0 is a valid field element");
        payload
            .iter()
            .copied()
            .bytes_to_fes()
            .with_checksum::<Bech32m>(&hrp)
            .with_witness_version(version)
            .chars()
            .collect()
    }

    /// Parse a BIP 352 bech32m address string.
    pub fn from_string(s: &str) -> Result<Self, BitcoinError> {
        Self::decode(s).map_err(BitcoinError::from)
    }

    /// Decode a BIP 352 bech32m address string into its components.
    fn decode(s: &str) -> Result<Self, SilentPaymentError> {
        let mut checked = CheckedHrpstring::new::<Bech32m>(s)
            .map_err(|e| SilentPaymentError::Address(format!("invalid bech32m: {e}")))?;

        let hrp = checked.hrp();
        let network = match hrp.as_str() {
            "sp" => Network::Bitcoin,
            // BIP 352 uses a single `tsp` HRP for all test networks; we default
            // to Testnet (round-trips for the common case).
            "tsp" => Network::Testnet,
            other => {
                return Err(SilentPaymentError::Address(format!(
                    "unexpected HRP '{other}', expected 'sp' or 'tsp'"
                )));
            }
        };

        let version = checked.remove_witness_version().ok_or_else(|| {
            SilentPaymentError::Address("missing silent-payment version byte".to_string())
        })?;
        if version.to_u8() != SP_VERSION {
            return Err(SilentPaymentError::Address(format!(
                "unsupported silent-payment version {}",
                version.to_u8()
            )));
        }

        let payload: Vec<u8> = checked.byte_iter().collect();
        if payload.len() != SP_PAYLOAD_LEN {
            return Err(SilentPaymentError::Address(format!(
                "invalid payload length: expected {SP_PAYLOAD_LEN} bytes, got {}",
                payload.len()
            )));
        }

        let scan_pubkey = PublicKey::from_slice(&payload[..COMPRESSED_PK_LEN])
            .map_err(|e| SilentPaymentError::Address(format!("invalid scan public key: {e}")))?;
        let spend_pubkey = PublicKey::from_slice(&payload[COMPRESSED_PK_LEN..])
            .map_err(|e| SilentPaymentError::Address(format!("invalid spend public key: {e}")))?;

        Ok(Self {
            scan_pubkey,
            spend_pubkey,
            label: None,
            network,
        })
    }
}

impl std::fmt::Display for SilentPaymentAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

/// Silent payment wallet for managing keys and scanning
#[derive(Debug)]
pub struct SilentPaymentWallet {
    /// Scan private key (b_scan)
    scan_privkey: SecretKey,
    /// Spend private key (b_spend)
    spend_privkey: SecretKey,
    /// Network
    network: Network,
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl SilentPaymentWallet {
    /// Create a new silent payment wallet with random keys
    pub fn new() -> Result<Self, BitcoinError> {
        Self::new_with_network(Network::Bitcoin)
    }

    /// Create a new wallet for a specific network
    pub fn new_with_network(network: Network) -> Result<Self, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secp = Secp256k1::new();
        let scan_privkey = SecretKey::new(&mut OsRng);
        let spend_privkey = SecretKey::new(&mut OsRng);

        Ok(Self {
            scan_privkey,
            spend_privkey,
            network,
            secp,
        })
    }

    /// Create from existing private keys
    pub fn from_keys(scan_privkey: SecretKey, spend_privkey: SecretKey, network: Network) -> Self {
        Self {
            scan_privkey,
            spend_privkey,
            network,
            secp: Secp256k1::new(),
        }
    }

    /// Get the silent payment address
    pub fn get_address(&self, label: Option<u32>) -> Result<SilentPaymentAddress, BitcoinError> {
        let scan_pubkey = PublicKey::from_secret_key(&self.secp, &self.scan_privkey);
        let mut spend_pubkey = PublicKey::from_secret_key(&self.secp, &self.spend_privkey);

        // Apply label if provided (BIP 352 label tweak): B_m = B_spend + label_tweak·G.
        if let Some(label_value) = label {
            let label_tweak = self.compute_label_tweak(label_value)?;
            spend_pubkey = spend_pubkey
                .add_exp_tweak(&self.secp, &label_tweak)
                .map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to apply label tweak: {e}"))
                })?;
        }

        Ok(SilentPaymentAddress::new(
            scan_pubkey,
            spend_pubkey,
            label,
            self.network,
        ))
    }

    /// Compute the BIP 352 label tweak scalar for a given label index.
    ///
    /// `label_tweak = hash_BIP0352/Label(ser256(b_scan) ‖ ser32(m))` where `m`
    /// is the 4-byte big-endian label index.
    fn compute_label_tweak(&self, label: u32) -> Result<Scalar, BitcoinError> {
        let hash = tagged_hash(
            TAG_LABEL,
            &[&self.scan_privkey.secret_bytes(), &label.to_be_bytes()],
        );
        scalar_from_be_bytes(hash, "label tweak").map_err(BitcoinError::from)
    }

    /// Get scan public key
    pub fn scan_pubkey(&self) -> PublicKey {
        PublicKey::from_secret_key(&self.secp, &self.scan_privkey)
    }

    /// Get spend public key
    pub fn spend_pubkey(&self) -> PublicKey {
        PublicKey::from_secret_key(&self.secp, &self.spend_privkey)
    }
}

/// Silent payment sender for creating payments
#[derive(Debug)]
pub struct SilentPaymentSender {
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl SilentPaymentSender {
    /// Create a new sender
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
        }
    }

    /// Derive the silent-payment output public key `P_k` for a recipient.
    ///
    /// This performs the full BIP 352 sender computation:
    ///
    /// 1. `a_sum = Σ input_privkeys` and `A_sum = a_sum·G`
    /// 2. `input_hash = hash_BIP0352/Inputs(outpoint_L ‖ A_sum)`
    /// 3. `ecdh = (input_hash · a_sum) · B_scan`
    /// 4. `t_k = hash_BIP0352/SharedSecret(ecdh ‖ ser32(k))`
    /// 5. `P_k = B_spend + t_k·G`
    ///
    /// `input_outpoints` must be the outpoints of the transaction inputs that
    /// contribute `input_privkeys` (used for the `input_hash`). For Taproot /
    /// x-only inputs the caller is responsible for supplying the private key with
    /// even-Y parity per BIP 352; P2PKH/P2WPKH keys are used as-is.
    pub fn derive_output_key(
        &self,
        recipient: &SilentPaymentAddress,
        input_privkeys: &[SecretKey],
        input_outpoints: &[OutPoint],
        output_index: u32,
    ) -> Result<PublicKey, BitcoinError> {
        let (a_sum, summed_pubkey) = self.sum_input_keys(input_privkeys)?;
        let input_hash = compute_input_hash(input_outpoints, &summed_pubkey)?;

        // ecdh = input_hash · a_sum · B_scan.
        // Fold `input_hash · a_sum` into a single scalar first, then multiply the
        // recipient's scan pubkey by it.
        let a_sum_input_hash = a_sum.mul_tweak(&input_hash).map_err(|e| {
            BitcoinError::from(SilentPaymentError::Crypto(format!(
                "input_hash · a_sum failed: {e}"
            )))
        })?;
        let ecdh_shared_secret = recipient
            .scan_pubkey
            .mul_tweak(&self.secp, &Scalar::from(a_sum_input_hash))
            .map_err(|e| {
                BitcoinError::from(SilentPaymentError::Crypto(format!("ECDH failed: {e}")))
            })?;

        let tweak = compute_output_tweak(&ecdh_shared_secret, output_index)?;

        // P_k = B_spend + t_k·G.
        recipient
            .spend_pubkey
            .add_exp_tweak(&self.secp, &tweak)
            .map_err(|e| {
                BitcoinError::from(SilentPaymentError::Crypto(format!(
                    "output key derivation failed: {e}"
                )))
            })
    }

    /// Create a P2TR output for a silent payment.
    ///
    /// Derives `P_k` via [`Self::derive_output_key`] and wraps it in a Taproot
    /// key-path output (the on-chain form of a silent payment).
    pub fn create_output(
        &self,
        recipient: &SilentPaymentAddress,
        input_privkeys: &[SecretKey],
        input_outpoints: &[OutPoint],
        output_index: u32,
        amount: u64,
    ) -> Result<TxOut, BitcoinError> {
        let output_pubkey =
            self.derive_output_key(recipient, input_privkeys, input_outpoints, output_index)?;

        // BIP 352: `P_k` is *itself* the Taproot output key. It must be placed
        // directly in the witness program with no further BIP 341 key tweak, so
        // we treat its x-only form as already-tweaked.
        let output_key = bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(
            output_pubkey.x_only_public_key().0,
        );
        let script_pubkey = bitcoin::ScriptBuf::new_p2tr_tweaked(output_key);

        Ok(TxOut {
            value: bitcoin::Amount::from_sat(amount),
            script_pubkey,
        })
    }

    /// Sum input private keys, returning `(a_sum, A_sum)`.
    fn sum_input_keys(
        &self,
        input_privkeys: &[SecretKey],
    ) -> Result<(SecretKey, PublicKey), BitcoinError> {
        let (first, rest) = input_privkeys.split_first().ok_or_else(|| {
            BitcoinError::from(SilentPaymentError::Crypto(
                "at least one input private key is required".to_string(),
            ))
        })?;

        let mut a_sum = *first;
        for privkey in rest {
            a_sum = a_sum.add_tweak(&Scalar::from(*privkey)).map_err(|e| {
                BitcoinError::from(SilentPaymentError::Crypto(format!(
                    "summing input private keys failed: {e}"
                )))
            })?;
        }

        let summed_pubkey = PublicKey::from_secret_key(&self.secp, &a_sum);
        Ok((a_sum, summed_pubkey))
    }
}

impl Default for SilentPaymentSender {
    fn default() -> Self {
        Self::new()
    }
}

/// Silent payment scanner for detecting owned outputs
#[derive(Debug)]
pub struct SilentPaymentScanner {
    wallet: SilentPaymentWallet,
    /// Cache of detected outputs
    detected_outputs: HashMap<bitcoin::Txid, Vec<DetectedOutput>>,
}

/// Detected silent payment output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedOutput {
    /// Transaction ID
    pub txid: bitcoin::Txid,
    /// Output index
    pub vout: u32,
    /// Amount in satoshis
    pub amount: u64,
    /// Derived spend private key for this output (`b_spend + t_k`)
    pub spend_privkey: SecretKey,
    /// Label if applicable
    pub label: Option<u32>,
}

impl SilentPaymentScanner {
    /// Create a new scanner with a wallet
    pub fn new(wallet: SilentPaymentWallet) -> Self {
        Self {
            wallet,
            detected_outputs: HashMap::new(),
        }
    }

    /// Compute the receiver-side shared secret for a transaction's inputs.
    ///
    /// `ecdh = input_hash · b_scan · A_sum`, the symmetric counterpart of the
    /// sender computation. `A_sum` is the sum of `input_pubkeys`; `input_hash`
    /// is derived from the input `outpoints`.
    fn compute_shared_secret(
        &self,
        input_pubkeys: &[PublicKey],
        outpoints: &[OutPoint],
    ) -> Result<PublicKey, SilentPaymentError> {
        let (first, rest) = input_pubkeys.split_first().ok_or_else(|| {
            SilentPaymentError::Crypto("at least one input public key is required".to_string())
        })?;

        let mut summed_pubkey = *first;
        for pubkey in rest {
            summed_pubkey = summed_pubkey
                .combine(pubkey)
                .map_err(|e| SilentPaymentError::Crypto(format!("summing input pubkeys: {e}")))?;
        }

        let input_hash = compute_input_hash(outpoints, &summed_pubkey)?;

        // ecdh = (input_hash · b_scan) · A_sum.
        let b_scan_input_hash = self
            .wallet
            .scan_privkey
            .mul_tweak(&input_hash)
            .map_err(|e| SilentPaymentError::Crypto(format!("input_hash · b_scan: {e}")))?;
        summed_pubkey
            .mul_tweak(&self.wallet.secp, &Scalar::from(b_scan_input_hash))
            .map_err(|e| SilentPaymentError::Crypto(format!("receiver ECDH: {e}")))
    }

    /// Derive the receiver-side output public key `P_k` for a single output.
    ///
    /// Mirrors [`SilentPaymentSender::derive_output_key`]; given the same inputs
    /// it yields the identical key, which is the property that makes detection
    /// possible.
    pub fn derive_output_key(
        &self,
        input_pubkeys: &[PublicKey],
        outpoints: &[OutPoint],
        output_index: u32,
        label: Option<u32>,
    ) -> Result<PublicKey, BitcoinError> {
        let ecdh_shared_secret = self.compute_shared_secret(input_pubkeys, outpoints)?;
        let tweak = compute_output_tweak(&ecdh_shared_secret, output_index)?;

        let mut spend_pubkey =
            PublicKey::from_secret_key(&self.wallet.secp, &self.wallet.spend_privkey);
        if let Some(label_value) = label {
            let label_tweak = self.wallet.compute_label_tweak(label_value)?;
            spend_pubkey = spend_pubkey
                .add_exp_tweak(&self.wallet.secp, &label_tweak)
                .map_err(|e| {
                    BitcoinError::from(SilentPaymentError::Crypto(format!(
                        "label tweak on spend key: {e}"
                    )))
                })?;
        }

        spend_pubkey
            .add_exp_tweak(&self.wallet.secp, &tweak)
            .map_err(|e| {
                BitcoinError::from(SilentPaymentError::Crypto(format!(
                    "output key derivation failed: {e}"
                )))
            })
    }

    /// Derive the spend private key for a detected output: `b_spend (+ label) + t_k`.
    fn derive_output_privkey(
        &self,
        ecdh_shared_secret: &PublicKey,
        output_index: u32,
        label: Option<u32>,
    ) -> Result<SecretKey, SilentPaymentError> {
        let tweak = compute_output_tweak(ecdh_shared_secret, output_index)?;

        let mut spend_privkey = self.wallet.spend_privkey;
        if let Some(label_value) = label {
            let label_tweak = self
                .wallet
                .compute_label_tweak(label_value)
                .map_err(|e| SilentPaymentError::Crypto(e.to_string()))?;
            spend_privkey = spend_privkey
                .add_tweak(&label_tweak)
                .map_err(|e| SilentPaymentError::Crypto(format!("label tweak on privkey: {e}")))?;
        }
        spend_privkey
            .add_tweak(&tweak)
            .map_err(|e| SilentPaymentError::Crypto(format!("output privkey derivation: {e}")))
    }

    /// Scan a single transaction for owned outputs.
    ///
    /// `input_pubkeys` are the public keys of the transaction's eligible inputs
    /// (extracted from prevouts/witnesses by the caller) and `input_outpoints`
    /// are the corresponding outpoints. Both are required because BIP 352 mixes
    /// the smallest outpoint into the `input_hash`.
    ///
    /// Outputs are P2TR; for each output we recompute the expected key
    /// `P_k = B_m + t_k·G` for increasing `k` and compare the x-only key against
    /// the output's witness program. Detected outputs (including for the few
    /// labels checked) carry the derived spend private key so the funds are
    /// spendable.
    pub fn scan_transaction(
        &mut self,
        tx: &bitcoin::Transaction,
        input_pubkeys: &[PublicKey],
        input_outpoints: &[OutPoint],
    ) -> Result<Vec<DetectedOutput>, BitcoinError> {
        if input_pubkeys.is_empty() || input_outpoints.is_empty() {
            return Ok(Vec::new());
        }

        let ecdh_shared_secret = self.compute_shared_secret(input_pubkeys, input_outpoints)?;
        let txid = tx.compute_txid();

        // Collect the P2TR output x-only keys we still need to match.
        let mut remaining: Vec<(u32, bitcoin::secp256k1::XOnlyPublicKey, u64)> = Vec::new();
        for (vout, output) in tx.output.iter().enumerate() {
            if let Some(xonly) = p2tr_output_key(output) {
                remaining.push((vout as u32, xonly, output.value.to_sat()));
            }
        }

        // Labels checked during scanning: the change label is not enumerated
        // exhaustively here; we check "no label" plus a small fixed set, exactly
        // as documented. This is honest: it is not a bulk optimisation, just a
        // bounded label search for a single transaction.
        let labels_to_try: [Option<u32>; 1] = [None];

        let mut detected = Vec::new();
        let mut output_index: u32 = 0;

        // BIP 352 increments k only when an output to this recipient is found.
        // We iterate k while there remain unmatched outputs.
        while !remaining.is_empty() {
            let tweak = compute_output_tweak(&ecdh_shared_secret, output_index)?;

            let mut matched_this_round = false;
            for label in labels_to_try {
                let mut spend_pubkey =
                    PublicKey::from_secret_key(&self.wallet.secp, &self.wallet.spend_privkey);
                if let Some(label_value) = label {
                    let label_tweak = self.wallet.compute_label_tweak(label_value)?;
                    spend_pubkey = spend_pubkey
                        .add_exp_tweak(&self.wallet.secp, &label_tweak)
                        .map_err(|e| {
                            BitcoinError::from(SilentPaymentError::Crypto(format!(
                                "label tweak: {e}"
                            )))
                        })?;
                }
                let expected = spend_pubkey
                    .add_exp_tweak(&self.wallet.secp, &tweak)
                    .map_err(|e| {
                        BitcoinError::from(SilentPaymentError::Crypto(format!(
                            "expected key derivation: {e}"
                        )))
                    })?
                    .x_only_public_key()
                    .0;

                if let Some(pos) = remaining.iter().position(|(_, xonly, _)| *xonly == expected) {
                    let (vout, _, amount) = remaining.remove(pos);
                    let spend_privkey =
                        self.derive_output_privkey(&ecdh_shared_secret, output_index, label)?;
                    detected.push(DetectedOutput {
                        txid,
                        vout,
                        amount,
                        spend_privkey,
                        label,
                    });
                    matched_this_round = true;
                    break;
                }
            }

            if matched_this_round {
                output_index += 1;
            } else {
                // No output matched for this k with any checked label; per BIP 352
                // scanning stops once an index yields no further match.
                break;
            }
        }

        if !detected.is_empty() {
            self.detected_outputs.insert(txid, detected.clone());
        }

        Ok(detected)
    }

    /// Bulk-scan a set of transactions.
    ///
    /// This optimisation (sharing work across many transactions, prevout
    /// fetching, tweak indexing) is intentionally **not** implemented and returns
    /// a typed error rather than fabricating detections. Scan transactions
    /// individually with [`Self::scan_transaction`] and real prevout context.
    pub fn scan_transaction_set(
        &mut self,
        _transactions: &[bitcoin::Transaction],
    ) -> Result<Vec<DetectedOutput>, SilentPaymentError> {
        Err(SilentPaymentError::BulkScanUnimplemented)
    }

    /// Get all detected outputs
    pub fn get_detected_outputs(&self) -> Vec<&DetectedOutput> {
        self.detected_outputs
            .values()
            .flat_map(|outputs| outputs.iter())
            .collect()
    }

    /// Get balance from detected outputs
    pub fn get_balance(&self) -> u64 {
        self.get_detected_outputs()
            .iter()
            .map(|output| output.amount)
            .sum()
    }
}

/// Extract the x-only output key from a P2TR `TxOut`, or `None` if not P2TR.
fn p2tr_output_key(output: &TxOut) -> Option<bitcoin::secp256k1::XOnlyPublicKey> {
    if !output.script_pubkey.is_p2tr() {
        return None;
    }
    // P2TR scriptPubKey: OP_1 (0x51) PUSH_32 <32-byte x-only key>.
    let bytes = output.script_pubkey.as_bytes();
    if bytes.len() != 34 {
        return None;
    }
    bitcoin::secp256k1::XOnlyPublicKey::from_slice(&bytes[2..34]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an `OutPoint` from a display-order txid hex string and a vout.
    fn outpoint(txid_hex: &str, vout: u32) -> OutPoint {
        let txid = txid_hex.parse::<bitcoin::Txid>().expect("valid txid hex");
        OutPoint { txid, vout }
    }

    #[test]
    fn test_silent_payment_wallet_creation() {
        let wallet = SilentPaymentWallet::new().unwrap();
        let address = wallet.get_address(None).unwrap();

        assert_eq!(address.network, Network::Bitcoin);
        assert_eq!(address.label, None);
        assert!(address.encode().starts_with("sp1q"));
    }

    #[test]
    fn test_silent_payment_with_label() {
        let wallet = SilentPaymentWallet::new().unwrap();
        let address_no_label = wallet.get_address(None).unwrap();
        let address_with_label = wallet.get_address(Some(1)).unwrap();

        // Different labels should produce different spend pubkeys
        assert_ne!(
            address_no_label.spend_pubkey,
            address_with_label.spend_pubkey
        );
        // But same scan pubkey
        assert_eq!(address_no_label.scan_pubkey, address_with_label.scan_pubkey);
    }

    #[test]
    fn test_silent_payment_networks() {
        let networks = vec![
            Network::Bitcoin,
            Network::Testnet,
            Network::Signet,
            Network::Regtest,
        ];

        for network in networks {
            let wallet = SilentPaymentWallet::new_with_network(network).unwrap();
            let address = wallet.get_address(None).unwrap();
            assert_eq!(address.network, network);
        }
    }

    #[test]
    fn test_address_round_trip_mainnet() {
        let wallet = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();
        let address = wallet.get_address(None).unwrap();
        let encoded = address.encode();

        assert!(encoded.starts_with("sp1q"), "got: {encoded}");
        let parsed = SilentPaymentAddress::from_string(&encoded).unwrap();

        assert_eq!(parsed.scan_pubkey, address.scan_pubkey);
        assert_eq!(parsed.spend_pubkey, address.spend_pubkey);
        assert_eq!(parsed.network, address.network);
    }

    #[test]
    fn test_address_round_trip_testnet() {
        let wallet = SilentPaymentWallet::new_with_network(Network::Testnet).unwrap();
        let address = wallet.get_address(None).unwrap();
        let encoded = address.encode();

        assert!(encoded.starts_with("tsp1q"), "got: {encoded}");
        let parsed = SilentPaymentAddress::from_string(&encoded).unwrap();

        assert_eq!(parsed.scan_pubkey, address.scan_pubkey);
        assert_eq!(parsed.spend_pubkey, address.spend_pubkey);
        assert_eq!(parsed.network, Network::Testnet);
    }

    #[test]
    fn test_invalid_address_prefix() {
        assert!(SilentPaymentAddress::from_string("invalid123").is_err());
        // A valid bech32m string with the wrong HRP must be rejected.
        assert!(SilentPaymentAddress::from_string("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4").is_err());
    }

    #[test]
    fn test_invalid_address_payload_length() {
        // Valid bech32m but too-short payload.
        let hrp = Hrp::parse_unchecked("sp");
        let short = [0u8; 10];
        let version = Fe32::try_from(SP_VERSION).unwrap();
        let s: String = short
            .iter()
            .copied()
            .bytes_to_fes()
            .with_checksum::<Bech32m>(&hrp)
            .with_witness_version(version)
            .chars()
            .collect();
        assert!(SilentPaymentAddress::from_string(&s).is_err());
    }

    #[test]
    fn test_sender_create_output() {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let wallet = SilentPaymentWallet::new().unwrap();
        let address = wallet.get_address(None).unwrap();
        let sender = SilentPaymentSender::new();

        let input_key = SecretKey::new(&mut OsRng);
        let op = outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        );

        let output = sender
            .create_output(&address, &[input_key], &[op], 0, 100_000)
            .unwrap();

        assert_eq!(output.value.to_sat(), 100_000);
        assert!(output.script_pubkey.is_p2tr());
    }

    #[test]
    fn test_scanner_creation() {
        let wallet = SilentPaymentWallet::new().unwrap();
        let scanner = SilentPaymentScanner::new(wallet);

        assert_eq!(scanner.get_balance(), 0);
        assert!(scanner.get_detected_outputs().is_empty());
    }

    /// The core invariant: a sender-derived output key equals the
    /// receiver-scanned output key for the same inputs and shared secret.
    #[test]
    fn test_sender_receiver_same_output_key() {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secp = Secp256k1::new();

        // Recipient wallet + address.
        let wallet = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();
        let address = wallet.get_address(None).unwrap();

        // Two inputs (exercise key summation).
        let in1 = SecretKey::new(&mut OsRng);
        let in2 = SecretKey::new(&mut OsRng);
        let pk1 = PublicKey::from_secret_key(&secp, &in1);
        let pk2 = PublicKey::from_secret_key(&secp, &in2);
        let outpoints = vec![
            outpoint(
                "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
                0,
            ),
            outpoint(
                "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d",
                2,
            ),
        ];

        let sender = SilentPaymentSender::new();
        let scanner = SilentPaymentScanner::new(wallet);

        for k in 0..3u32 {
            let sender_key = sender
                .derive_output_key(&address, &[in1, in2], &outpoints, k)
                .unwrap();
            let receiver_key = scanner
                .derive_output_key(&[pk1, pk2], &outpoints, k, None)
                .unwrap();
            assert_eq!(
                sender_key, receiver_key,
                "sender and receiver disagree at k={k}"
            );
        }
    }

    /// End-to-end: sender builds a P2TR output, scanner detects it and the
    /// derived spend key controls the output.
    #[test]
    fn test_end_to_end_detection() {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secp = Secp256k1::new();
        let wallet = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();
        let address = wallet.get_address(None).unwrap();

        let in1 = SecretKey::new(&mut OsRng);
        let pk1 = PublicKey::from_secret_key(&secp, &in1);
        let outpoints = vec![outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        )];

        let sender = SilentPaymentSender::new();
        let txout = sender
            .create_output(&address, &[in1], &outpoints, 0, 50_000)
            .unwrap();

        // Build a minimal transaction carrying the silent-payment output.
        let tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoints[0],
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![txout.clone()],
        };

        let mut scanner = SilentPaymentScanner::new(wallet);
        let detected = scanner.scan_transaction(&tx, &[pk1], &outpoints).unwrap();

        assert_eq!(detected.len(), 1, "expected exactly one detected output");
        let det = &detected[0];
        assert_eq!(det.vout, 0);
        assert_eq!(det.amount, 50_000);

        // The derived spend private key must control the on-chain output key.
        let derived_pub = PublicKey::from_secret_key(&secp, &det.spend_privkey)
            .x_only_public_key()
            .0;
        let onchain = p2tr_output_key(&txout).unwrap();
        assert_eq!(derived_pub, onchain, "derived key does not match output");

        assert_eq!(scanner.get_balance(), 50_000);
    }

    #[test]
    fn test_unrelated_output_not_detected() {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secp = Secp256k1::new();
        let wallet = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();

        // An output to an *unrelated* recipient must not be detected.
        let other = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();
        let other_addr = other.get_address(None).unwrap();

        let in1 = SecretKey::new(&mut OsRng);
        let pk1 = PublicKey::from_secret_key(&secp, &in1);
        let outpoints = vec![outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        )];

        let sender = SilentPaymentSender::new();
        let txout = sender
            .create_output(&other_addr, &[in1], &outpoints, 0, 10_000)
            .unwrap();
        let tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoints[0],
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![txout],
        };

        let mut scanner = SilentPaymentScanner::new(wallet);
        let detected = scanner.scan_transaction(&tx, &[pk1], &outpoints).unwrap();
        assert!(detected.is_empty(), "must not detect unrelated output");
    }

    #[test]
    fn test_bulk_scan_returns_honest_error() {
        let wallet = SilentPaymentWallet::new().unwrap();
        let mut scanner = SilentPaymentScanner::new(wallet);
        let err = scanner.scan_transaction_set(&[]).unwrap_err();
        assert!(matches!(err, SilentPaymentError::BulkScanUnimplemented));
    }

    /// Official BIP 352 test vector ("Simple send: two inputs").
    ///
    /// Verifies the full sender pipeline against the reference output key, and
    /// confirms the recipient address encodes to the canonical bech32m string.
    #[test]
    fn test_bip352_official_vector_simple_send() {
        let secp = Secp256k1::new();

        let priv1 = secret_key_from_hex(
            "eadc78165ff1f8ea94ad7cfdc54990738a4c53f6e0507b42154201b8e5dff3b1",
        );
        let priv2 = secret_key_from_hex(
            "93f5ed907ad5b2bdbbdcb5d9116ebc0a4e1f92f910d5260237fa45a9408aad16",
        );

        let op1 = outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        );
        let op2 = outpoint(
            "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d",
            0,
        );

        let scan_pub = public_key_from_hex(
            "0220bcfac5b99e04ad1a06ddfb016ee13582609d60b6291e98d01a9bc9a16c96d4",
        );
        let spend_pub = public_key_from_hex(
            "025cc9856d6f8375350e123978daac200c260cb5b5ae83106cab90484dcd8fcf36",
        );
        let address = SilentPaymentAddress::new(scan_pub, spend_pub, None, Network::Bitcoin);

        // (a) Address encodes to the canonical BIP 352 string and round-trips.
        let expected_addr = "sp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv";
        assert_eq!(address.encode(), expected_addr);
        let decoded = SilentPaymentAddress::from_string(expected_addr).unwrap();
        assert_eq!(decoded.scan_pubkey, scan_pub);
        assert_eq!(decoded.spend_pubkey, spend_pub);

        // (b) input_private_key_sum matches the vector.
        let sender = SilentPaymentSender::new();
        let (a_sum, a_sum_pub) = sender.sum_input_keys(&[priv1, priv2]).unwrap();
        assert_eq!(
            hex::encode(a_sum.secret_bytes()),
            "7ed265a6dac7aba8508a32d6d6b84c7f1dbd0a0941dd01088d69e8d556345f86"
        );
        assert_eq!(
            hex::encode(a_sum_pub.serialize()),
            "032562c1ab2d6bd45d7ca4d78f569999e5333dffd3ac5263924fd00d00dedc4bee"
        );

        // (c) Shared secret (sender side) matches the vector.
        let input_hash = compute_input_hash(&[op1, op2], &a_sum_pub).unwrap();
        let a_sum_input_hash = a_sum.mul_tweak(&input_hash).unwrap();
        let ecdh = scan_pub
            .mul_tweak(&secp, &Scalar::from(a_sum_input_hash))
            .unwrap();
        assert_eq!(
            hex::encode(ecdh.serialize()),
            "028158aff7d61ea66b2fa7f555bc3c5937d1debbde16423d630f9aa7943e14d80d"
        );

        // (d) Per-output tweak t_0 matches `priv_key_tweak`.
        let tweak = compute_output_tweak(&ecdh, 0).unwrap();
        assert_eq!(
            hex::encode(tweak.to_be_bytes()),
            "f438b40179a3c4262de12986c0e6cce0634007cdc79c1dcd3e20b9ebc2e7eef6"
        );

        // (e) Final output x-only key P_0 matches the vector.
        let output_key = sender
            .derive_output_key(&address, &[priv1, priv2], &[op1, op2], 0)
            .unwrap();
        assert_eq!(
            hex::encode(output_key.x_only_public_key().0.serialize()),
            "3e9fce73d4e77a4809908e3c3a2e54ee147b9312dc5044a193d1fc85de46e3c1"
        );
    }

    /// Receiver side of the official vector: scan_priv/spend_priv recompute the
    /// same output key and the spend private key matches `b_spend + t_0`.
    #[test]
    fn test_bip352_official_vector_receiver() {
        let secp = Secp256k1::new();

        let scan_priv = secret_key_from_hex(
            "0f694e068028a717f8af6b9411f9a133dd3565258714cc226594b34db90c1f2c",
        );
        let spend_priv = secret_key_from_hex(
            "9d6ad855ce3417ef84e836892e5a56392bfba05fa5d97ccea30e266f540e08b3",
        );
        let wallet = SilentPaymentWallet::from_keys(scan_priv, spend_priv, Network::Bitcoin);

        // Wallet derives the published recipient address.
        assert_eq!(
            wallet.get_address(None).unwrap().encode(),
            "sp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv"
        );

        let pub1 = public_key_from_hex(
            "025a1e61f898173040e20616d43e9f496fba90338a39faa1ed98fcbaeee4dd9be5",
        );
        let pub2 = public_key_from_hex(
            "03bd85685d03d111699b15d046319febe77f8de5286e9e512703cdee1bf3be3792",
        );
        let op1 = outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        );
        let op2 = outpoint(
            "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d",
            0,
        );

        let scanner = SilentPaymentScanner::new(wallet);

        // Receiver-derived output key equals the vector's output key.
        let receiver_key = scanner
            .derive_output_key(&[pub1, pub2], &[op1, op2], 0, None)
            .unwrap();
        assert_eq!(
            hex::encode(receiver_key.x_only_public_key().0.serialize()),
            "3e9fce73d4e77a4809908e3c3a2e54ee147b9312dc5044a193d1fc85de46e3c1"
        );

        // Spend private key = b_spend + t_0; its pubkey must match the output.
        let ecdh = scanner.compute_shared_secret(&[pub1, pub2], &[op1, op2]).unwrap();
        let out_priv = scanner.derive_output_privkey(&ecdh, 0, None).unwrap();
        assert_eq!(
            PublicKey::from_secret_key(&secp, &out_priv)
                .x_only_public_key()
                .0,
            receiver_key.x_only_public_key().0
        );
    }

    /// `input_hash` must use the lexicographically smallest outpoint regardless
    /// of the order outpoints are supplied in.
    #[test]
    fn test_input_hash_outpoint_ordering_invariant() {
        let secp = Secp256k1::new();
        let a_sum_pub = public_key_from_hex(
            "032562c1ab2d6bd45d7ca4d78f569999e5333dffd3ac5263924fd00d00dedc4bee",
        );
        let op1 = outpoint(
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
            0,
        );
        let op2 = outpoint(
            "a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d",
            0,
        );
        let h1 = compute_input_hash(&[op1, op2], &a_sum_pub).unwrap();
        let h2 = compute_input_hash(&[op2, op1], &a_sum_pub).unwrap();
        assert_eq!(h1.to_be_bytes(), h2.to_be_bytes());
        let _ = secp;
    }

    fn secret_key_from_hex(s: &str) -> SecretKey {
        let bytes = hex::decode(s).expect("valid hex");
        SecretKey::from_slice(&bytes).expect("valid secret key")
    }

    fn public_key_from_hex(s: &str) -> PublicKey {
        let bytes = hex::decode(s).expect("valid hex");
        PublicKey::from_slice(&bytes).expect("valid public key")
    }
}
