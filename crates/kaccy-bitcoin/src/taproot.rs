//! Taproot (BIP 341/342) support
//!
//! This module provides support for Taproot addresses and transactions,
//! enabling enhanced privacy, lower fees, and more flexible smart contracts.
//!
//! Spending is implemented against the `bitcoin` (0.32) + `secp256k1` (re-exported
//! via `bitcoin::secp256k1`) crates so that all key, signature and hash types stay
//! version-coherent with the `bitcoin` crate's internal `secp256k1` build.

use bitcoin::address::Address;
use bitcoin::blockdata::script::Instruction;
use bitcoin::hashes::Hash;
use bitcoin::key::{Secp256k1, TapTweak};
use bitcoin::secp256k1::rand::rngs::OsRng;
use bitcoin::secp256k1::{All, Keypair, Message, PublicKey, SecretKey, XOnlyPublicKey};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::{
    LeafVersion, Signature as TaprootSignature, TapLeafHash, TapNodeHash, TaprootBuilder,
    TaprootSpendInfo,
};
use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

use crate::error::{BitcoinError, Result};

/// Taproot configuration
#[derive(Debug, Clone)]
pub struct TaprootConfig {
    /// Bitcoin network
    pub network: Network,
    /// Enable script path spending
    pub enable_script_path: bool,
    /// Maximum script tree depth
    pub max_tree_depth: u8,
}

impl Default for TaprootConfig {
    fn default() -> Self {
        Self {
            network: Network::Bitcoin,
            enable_script_path: true,
            max_tree_depth: 128,
        }
    }
}

/// Taproot address manager
///
/// Manages Taproot addresses and transactions (BIP 341/342).
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::{TaprootManager, TaprootConfig};
/// use bitcoin::Network;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = TaprootConfig {
///     network: Network::Testnet,
///     enable_script_path: true,
///     max_tree_depth: 128,
/// };
///
/// let manager = TaprootManager::new(config);
///
/// // Generate a Taproot key pair
/// let keypair = manager.generate_keypair()?;
///
/// // Create a key-path address (most private and efficient)
/// let address = manager.create_key_path_address(keypair.internal_key)?;
/// println!("Taproot address: {}", address.address);
/// # Ok(())
/// # }
/// ```
pub struct TaprootManager {
    config: TaprootConfig,
    secp: Secp256k1<All>,
}

/// Taproot key pair
#[derive(Debug, Clone)]
pub struct TaprootKeyPair {
    /// Internal key (x-only public key)
    pub internal_key: XOnlyPublicKey,
    /// Secret key (for signing)
    secret_key: Option<SecretKey>,
}

impl TaprootKeyPair {
    /// Returns the secret key backing this key pair, if available.
    ///
    /// Key pairs derived from a public key alone (for example when reconstructed
    /// from an address) do not carry a secret key and therefore cannot sign.
    pub fn secret_key(&self) -> Option<&SecretKey> {
        self.secret_key.as_ref()
    }
}

/// Taproot script tree
#[derive(Debug, Clone)]
pub struct TaprootScriptTree {
    /// Root of the script tree
    pub root: Option<TapNodeHash>,
    /// Leaves in the tree
    pub leaves: Vec<TaprootScriptLeaf>,
}

/// A leaf in the Taproot script tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaprootScriptLeaf {
    /// The script for this leaf
    pub script: ScriptBuf,
    /// Leaf version (typically 0xc0 for Tapscript)
    pub version: u8,
    /// Leaf hash
    pub leaf_hash: TapLeafHash,
}

/// Taproot address with spending info
#[derive(Debug, Clone)]
pub struct TaprootAddress {
    /// The Bitcoin address
    pub address: Address,
    /// Spending information
    pub spend_info: TaprootSpendInfo,
    /// Internal key
    pub internal_key: XOnlyPublicKey,
}

/// Taproot spending path
#[derive(Debug, Clone)]
pub enum TaprootSpendPath {
    /// Key path (most private and efficient)
    KeyPath,
    /// Script path with specific leaf
    ScriptPath {
        /// Index of the script leaf to use
        leaf_index: usize,
    },
}

impl TaprootManager {
    /// Create a new Taproot manager
    pub fn new(config: TaprootConfig) -> Self {
        Self {
            config,
            secp: Secp256k1::new(),
        }
    }

    /// Generate a new Taproot key pair
    pub fn generate_keypair(&self) -> Result<TaprootKeyPair> {
        let secret_key = SecretKey::new(&mut OsRng);
        let public_key = PublicKey::from_secret_key(&self.secp, &secret_key);
        let (x_only_pubkey, _parity) = public_key.x_only_public_key();

        Ok(TaprootKeyPair {
            internal_key: x_only_pubkey,
            secret_key: Some(secret_key),
        })
    }

    /// Create a Taproot address from an internal key (key-path only)
    pub fn create_key_path_address(&self, internal_key: XOnlyPublicKey) -> Result<TaprootAddress> {
        // For key-path only, we create a simple taproot without script tree
        let builder = TaprootBuilder::new();
        let spend_info = builder
            .finalize(&self.secp, internal_key)
            .map_err(|_| BitcoinError::Validation("Taproot finalization failed".to_string()))?;

        let _output_key = spend_info.output_key();
        let address = Address::p2tr(&self.secp, internal_key, None, self.config.network);

        Ok(TaprootAddress {
            address,
            spend_info,
            internal_key,
        })
    }

    /// Create a Taproot address with script tree
    pub fn create_script_path_address(
        &self,
        internal_key: XOnlyPublicKey,
        scripts: Vec<ScriptBuf>,
    ) -> Result<TaprootAddress> {
        if !self.config.enable_script_path {
            return Err(BitcoinError::Validation(
                "Script path spending is disabled".to_string(),
            ));
        }

        if scripts.is_empty() {
            return Err(BitcoinError::Validation(
                "At least one script is required".to_string(),
            ));
        }

        // Build a balanced script tree. For a single leaf we place it at depth 0;
        // for multiple leaves we place them at a uniform depth that can hold them
        // (the smallest depth `d` with `2^d >= n`), padding is handled by the
        // builder's Huffman-free uniform layout via `add_leaf`.
        let spend_info = Self::build_spend_info(&self.secp, internal_key, &scripts)?;

        let merkle_root = spend_info.merkle_root();
        let address = Address::p2tr(&self.secp, internal_key, merkle_root, self.config.network);

        Ok(TaprootAddress {
            address,
            spend_info,
            internal_key,
        })
    }

    /// Build [`TaprootSpendInfo`] for the given internal key and ordered scripts.
    ///
    /// Each script is added as a tapscript leaf at a uniform depth large enough to
    /// hold every leaf, producing a deterministic, well-formed merkle tree.
    fn build_spend_info(
        secp: &Secp256k1<All>,
        internal_key: XOnlyPublicKey,
        scripts: &[ScriptBuf],
    ) -> Result<TaprootSpendInfo> {
        let mut builder = TaprootBuilder::new();

        if scripts.len() == 1 {
            builder = builder
                .add_leaf(0, scripts[0].clone())
                .map_err(|e| BitcoinError::Validation(format!("Failed to add leaf: {}", e)))?;
        } else {
            // Smallest depth whose full binary tree can hold all leaves.
            let depth = (usize::BITS - (scripts.len() - 1).leading_zeros()) as u8;
            for script in scripts {
                builder = builder
                    .add_leaf(depth, script.clone())
                    .map_err(|e| BitcoinError::Validation(format!("Failed to add leaf: {}", e)))?;
            }
        }

        builder
            .finalize(secp, internal_key)
            .map_err(|_| BitcoinError::Validation("Taproot finalization failed".to_string()))
    }

    /// Validate a Taproot address
    pub fn validate_address(&self, address: &str) -> Result<bool> {
        let addr = Address::from_str(address)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid address: {}", e)))?
            .require_network(self.config.network)
            .map_err(|_| BitcoinError::InvalidAddress("Network mismatch".to_string()))?;

        // Check if the address script_pubkey is P2TR (32 bytes + OP_1)
        Ok(addr.script_pubkey().is_p2tr())
    }

    /// Check if an address is a Taproot address
    pub fn is_taproot_address(&self, address: &Address) -> bool {
        address.script_pubkey().is_p2tr()
    }

    /// Extract the x-only output key committed in a Taproot address.
    ///
    /// # BIP-341 note
    ///
    /// A Taproot `scriptPubKey` (`OP_1 <32-byte x-only key>`) commits to the
    /// *tweaked output key* `Q = P + H_TapTweak(P || merkle_root) * G`, not to the
    /// internal key `P`. The internal key and the script-tree merkle root are
    /// **not** recoverable from an address alone, so this method returns the
    /// output key. For a key-path-only output (no script tree) the output key is
    /// `tweak(internal_key, None)`, which is the value you must verify Schnorr
    /// signatures against during a key-path spend.
    ///
    /// The key is decoded by walking the `scriptPubKey` with the `bitcoin` crate's
    /// typed instruction parser ([`Script::instructions`](bitcoin::Script::instructions))
    /// rather than ad-hoc byte slicing: a P2TR script is exactly
    /// `OP_PUSHNUM_1` followed by a single 32-byte push of the x-only output key.
    pub fn extract_internal_key(&self, address: &Address) -> Result<XOnlyPublicKey> {
        let script_pubkey = address.script_pubkey();
        if !script_pubkey.is_p2tr() {
            return Err(BitcoinError::InvalidAddress(
                "Address is not a Taproot address".to_string(),
            ));
        }

        // Walk the script: instruction[0] is OP_PUSHNUM_1 (witness v1),
        // instruction[1] is the 32-byte push carrying the x-only output key.
        let mut instructions = script_pubkey.instructions();
        let _version = instructions
            .next()
            .transpose()
            .map_err(|e| BitcoinError::InvalidAddress(format!("Malformed Taproot script: {}", e)))?
            .ok_or_else(|| BitcoinError::InvalidAddress("Empty Taproot script".to_string()))?;

        let program =
            match instructions.next().transpose().map_err(|e| {
                BitcoinError::InvalidAddress(format!("Malformed Taproot script: {}", e))
            })? {
                Some(Instruction::PushBytes(bytes)) => bytes,
                _ => {
                    return Err(BitcoinError::InvalidAddress(
                        "Taproot script missing key push".to_string(),
                    ));
                }
            };

        if program.len() != 32 {
            return Err(BitcoinError::InvalidAddress(format!(
                "Invalid Taproot witness program length: {}",
                program.len()
            )));
        }

        XOnlyPublicKey::from_slice(program.as_bytes())
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid x-only output key: {}", e)))
    }
}

/// Parameters describing the input being spent by [`TaprootTxBuilder`].
///
/// Spending a Taproot output requires more context than an address can carry:
/// the exact outpoint, the previous `TxOut` (its value and `scriptPubKey` are
/// committed by the BIP-341 sighash), and the signing key. This struct bundles
/// that context for the spend constructors.
#[derive(Debug, Clone)]
pub struct TaprootSpendInput {
    /// Outpoint (txid:vout) being spent.
    pub outpoint: OutPoint,
    /// The previous output (value + scriptPubKey) committed by the sighash.
    pub prev_output: TxOut,
    /// Secret key used to produce the Schnorr signature.
    pub secret_key: SecretKey,
}

impl TaprootSpendInput {
    /// Construct a spend input from a textual `txid:vout`, the previous output,
    /// and the signing secret key.
    pub fn new(txid: &str, vout: u32, prev_output: TxOut, secret_key: SecretKey) -> Result<Self> {
        let txid = Txid::from_str(txid)
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Invalid txid: {}", e)))?;
        Ok(Self {
            outpoint: OutPoint { txid, vout },
            prev_output,
            secret_key,
        })
    }
}

/// Taproot transaction builder
pub struct TaprootTxBuilder {
    manager: Arc<TaprootManager>,
}

impl TaprootTxBuilder {
    /// Create a new Taproot transaction builder
    pub fn new(manager: Arc<TaprootManager>) -> Self {
        Self { manager }
    }

    /// Build the unsigned single-input/single-output skeleton transaction.
    fn build_skeleton(
        spend: &TaprootSpendInput,
        destination: &Address,
        amount: Amount,
    ) -> Transaction {
        let txin = TxIn {
            previous_output: spend.outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let txout = TxOut {
            value: amount,
            script_pubkey: destination.script_pubkey(),
        };

        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![txout],
        }
    }

    /// Create a key-path spend of a Taproot output (BIP-341).
    ///
    /// This tweaks the internal key by `merkle_root` (`None` for key-path-only
    /// outputs), computes the BIP-341 key-spend sighash with `SIGHASH_DEFAULT`,
    /// produces a BIP-340 Schnorr signature with `secp256k1`, and attaches the
    /// resulting single-element witness. The returned transaction is fully
    /// signed and ready to broadcast.
    ///
    /// `merkle_root` must match the tree the output was created with so the
    /// tweaked key equals the on-chain output key; pass `None` for key-only.
    pub fn create_key_path_spend(
        &self,
        spend: TaprootSpendInput,
        destination: Address,
        amount: Amount,
        merkle_root: Option<TapNodeHash>,
    ) -> Result<Transaction> {
        let secp = &self.manager.secp;
        let mut tx = Self::build_skeleton(&spend, &destination, amount);

        // Tweak the internal key pair by the merkle root: q = p + H_TapTweak(P||c).
        let internal_keypair = Keypair::from_secret_key(secp, &spend.secret_key);
        let tweaked_keypair = internal_keypair.tap_tweak(secp, merkle_root).to_keypair();

        // BIP-341 key-spend sighash with SIGHASH_DEFAULT over the single prevout.
        let prevouts = [spend.prev_output.clone()];
        let mut sighash_cache = SighashCache::new(&tx);
        let sighash = sighash_cache
            .taproot_key_spend_signature_hash(0, &Prevouts::All(&prevouts), TapSighashType::Default)
            .map_err(|e| {
                BitcoinError::Validation(format!("Taproot key-spend sighash failed: {}", e))
            })?;

        // BIP-340 Schnorr signature over the sighash with the tweaked key.
        let message = Message::from_digest(sighash.to_byte_array());
        let signature = secp.sign_schnorr(&message, &tweaked_keypair);

        let taproot_sig = TaprootSignature {
            signature,
            sighash_type: TapSighashType::Default,
        };

        // Single 64-byte witness element for SIGHASH_DEFAULT (65 if non-default).
        tx.input[0].witness = Witness::p2tr_key_spend(&taproot_sig);

        Ok(tx)
    }

    /// Create a script-path spend of a Taproot output (BIP-341/342).
    ///
    /// Using the provided [`TaprootSpendInfo`], this resolves the control block
    /// for the chosen leaf (proving the leaf's membership in the committed tree),
    /// computes the BIP-341 tapscript sighash bound to that leaf, signs it with
    /// the internal key (`SIGHASH_DEFAULT`), and assembles the witness stack
    /// `[signature, script, control_block]`.
    ///
    /// `sighash_type` defaults to `SIGHASH_DEFAULT`; the leaf script chosen here
    /// is a simple `<pubkey> OP_CHECKSIG`-style tapscript that the internal key
    /// can satisfy, so a single Schnorr signature is the only script input.
    pub fn create_script_path_spend(
        &self,
        spend: TaprootSpendInput,
        destination: Address,
        amount: Amount,
        leaf_index: usize,
        spend_info: &TaprootSpendInfo,
        scripts: &[ScriptBuf],
    ) -> Result<Transaction> {
        if !self.manager.config.enable_script_path {
            return Err(BitcoinError::Validation(
                "Script path spending is disabled".to_string(),
            ));
        }

        let script = scripts.get(leaf_index).ok_or_else(|| {
            BitcoinError::Validation(format!(
                "Leaf index {} out of range ({} leaves)",
                leaf_index,
                scripts.len()
            ))
        })?;

        let secp = &self.manager.secp;
        let mut tx = Self::build_skeleton(&spend, &destination, amount);

        // Resolve the control block proving this leaf is committed in the tree.
        let leaf = (script.clone(), LeafVersion::TapScript);
        let control_block = spend_info.control_block(&leaf).ok_or_else(|| {
            BitcoinError::Validation(
                "Control block unavailable: leaf not in this Taproot tree".to_string(),
            )
        })?;

        // BIP-341 tapscript sighash bound to this specific leaf.
        let leaf_hash = TapLeafHash::from_script(script, LeafVersion::TapScript);
        let prevouts = [spend.prev_output.clone()];
        let mut sighash_cache = SighashCache::new(&tx);
        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&prevouts),
                leaf_hash,
                TapSighashType::Default,
            )
            .map_err(|e| {
                BitcoinError::Validation(format!("Taproot script-spend sighash failed: {}", e))
            })?;

        // Sign with the (untweaked) internal key — script-path signatures are not
        // tweaked; the script's OP_CHECKSIG verifies against the bare pubkey.
        let keypair = Keypair::from_secret_key(secp, &spend.secret_key);
        let message = Message::from_digest(sighash.to_byte_array());
        let signature = secp.sign_schnorr(&message, &keypair);
        let taproot_sig = TaprootSignature {
            signature,
            sighash_type: TapSighashType::Default,
        };

        // Witness stack: [..script_inputs, script, control_block].
        let mut witness = Witness::new();
        witness.push(taproot_sig.to_vec());
        witness.push(script.as_bytes());
        witness.push(control_block.serialize());
        tx.input[0].witness = witness;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::opcodes::all::OP_CHECKSIG;
    use bitcoin::script::Builder;
    use bitcoin::secp256k1::schnorr::Signature as SchnorrSignature;

    fn test_manager() -> TaprootManager {
        TaprootManager::new(TaprootConfig {
            network: Network::Testnet,
            ..Default::default()
        })
    }

    /// Build a `<pubkey> OP_CHECKSIG` tapscript for the given x-only key.
    fn checksig_script(key: &XOnlyPublicKey) -> ScriptBuf {
        Builder::new()
            .push_x_only_key(key)
            .push_opcode(OP_CHECKSIG)
            .into_script()
    }

    #[test]
    fn test_taproot_config_defaults() {
        let config = TaprootConfig::default();
        assert_eq!(config.network, Network::Bitcoin);
        assert!(config.enable_script_path);
        assert_eq!(config.max_tree_depth, 128);
    }

    #[test]
    fn test_generate_keypair() {
        let manager = test_manager();
        let keypair = manager.generate_keypair().expect("keypair generation");
        assert!(keypair.secret_key().is_some());
    }

    #[test]
    fn test_create_key_path_address() {
        let manager = test_manager();
        let keypair = manager.generate_keypair().expect("keypair generation");
        let address = manager
            .create_key_path_address(keypair.internal_key)
            .expect("key-path address");
        assert!(manager.is_taproot_address(&address.address));
    }

    #[test]
    fn test_create_script_path_address() {
        let manager = test_manager();
        let keypair = manager.generate_keypair().expect("keypair generation");
        let script = ScriptBuf::from_bytes(vec![0x51]); // OP_TRUE
        let address = manager
            .create_script_path_address(keypair.internal_key, vec![script])
            .expect("script-path address");
        assert!(manager.is_taproot_address(&address.address));
    }

    #[test]
    fn test_validate_taproot_address() {
        let manager = test_manager();
        let keypair = manager.generate_keypair().expect("keypair generation");
        let address = manager
            .create_key_path_address(keypair.internal_key)
            .expect("key-path address");
        let is_valid = manager
            .validate_address(&address.address.to_string())
            .expect("validation");
        assert!(is_valid);
    }

    #[test]
    fn test_script_path_disabled() {
        let config = TaprootConfig {
            network: Network::Testnet,
            enable_script_path: false,
            ..Default::default()
        };
        let manager = TaprootManager::new(config);
        let keypair = manager.generate_keypair().expect("keypair generation");
        let script = ScriptBuf::from_bytes(vec![0x51]);
        let result = manager.create_script_path_address(keypair.internal_key, vec![script]);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_output_key_roundtrips_for_key_only() {
        // For a key-path-only output the on-chain key is tweak(internal, None).
        let manager = test_manager();
        let keypair = manager.generate_keypair().expect("keypair generation");
        let address = manager
            .create_key_path_address(keypair.internal_key)
            .expect("key-path address");

        let extracted = manager
            .extract_internal_key(&address.address)
            .expect("extract output key");
        let expected = address.spend_info.output_key().to_x_only_public_key();
        assert_eq!(extracted, expected);
        // It is the *tweaked* key, not the raw internal key.
        assert_ne!(extracted, keypair.internal_key);
    }

    #[test]
    fn test_extract_internal_key_rejects_non_taproot() {
        let manager = test_manager();
        // A P2WPKH address is not Taproot.
        let secp = Secp256k1::new();
        let sk = SecretKey::new(&mut OsRng);
        let pk = bitcoin::PublicKey::new(PublicKey::from_secret_key(&secp, &sk));
        let wpkh = pk.wpubkey_hash().expect("compressed key");
        let address = Address::p2wpkh(
            &bitcoin::key::CompressedPublicKey(pk.inner),
            Network::Testnet,
        );
        let _ = wpkh; // hash computed to assert compression validity above
        let result = manager.extract_internal_key(&address);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_path_spend_signature_verifies() {
        // Build a key-path-only Taproot output, spend it, and verify the produced
        // Schnorr signature against the tweaked output key for the recomputed sighash.
        let manager = Arc::new(test_manager());
        let secp = Secp256k1::new();

        let secret_key = SecretKey::new(&mut OsRng);
        let internal_key = Keypair::from_secret_key(&secp, &secret_key)
            .x_only_public_key()
            .0;

        let taproot_address = manager
            .create_key_path_address(internal_key)
            .expect("key-path address");

        // Funding output we are spending (1.0 BTC).
        let prev_value = Amount::from_sat(100_000_000);
        let prev_output = TxOut {
            value: prev_value,
            script_pubkey: taproot_address.address.script_pubkey(),
        };
        let txid = "0000000000000000000000000000000000000000000000000000000000000001";
        let spend =
            TaprootSpendInput::new(txid, 0, prev_output.clone(), secret_key).expect("spend input");

        // Destination: a fresh key-path address.
        let dest_kp = manager.generate_keypair().expect("dest keypair");
        let dest = manager
            .create_key_path_address(dest_kp.internal_key)
            .expect("dest address")
            .address;

        let builder = TaprootTxBuilder::new(manager.clone());
        let fee = Amount::from_sat(1_000);
        let amount = prev_value - fee;
        let tx = builder
            .create_key_path_spend(spend, dest, amount, None)
            .expect("key-path spend");

        // Witness must be a single element (64 bytes for SIGHASH_DEFAULT).
        assert_eq!(tx.input.len(), 1);
        let witness = &tx.input[0].witness;
        assert_eq!(witness.len(), 1, "key-path witness must hold one element");
        let sig_bytes = witness.iter().next().expect("witness element");
        assert_eq!(sig_bytes.len(), 64, "SIGHASH_DEFAULT signature is 64 bytes");

        // Recompute the sighash exactly as a verifier would.
        let prevouts = [prev_output];
        let mut cache = SighashCache::new(&tx);
        let sighash = cache
            .taproot_key_spend_signature_hash(0, &Prevouts::All(&prevouts), TapSighashType::Default)
            .expect("recompute sighash");
        let message = Message::from_digest(sighash.to_byte_array());

        // The signature must verify against the TWEAKED output key.
        let output_key = taproot_address
            .spend_info
            .output_key()
            .to_x_only_public_key();
        let parsed_sig = SchnorrSignature::from_slice(sig_bytes).expect("parse schnorr sig");
        secp.verify_schnorr(&parsed_sig, &message, &output_key)
            .expect("key-path signature verifies against tweaked output key");

        // And it must NOT verify against the untweaked internal key.
        assert!(
            secp.verify_schnorr(&parsed_sig, &message, &internal_key)
                .is_err(),
            "signature must not verify against the untweaked internal key"
        );
    }

    #[test]
    fn test_script_path_spend_control_block_and_witness() {
        // Build a Taproot output with a script tree, perform a script-path spend,
        // and assert the control block verifies the leaf against the output key
        // and that the witness stack is well-formed.
        let manager = Arc::new(test_manager());
        let secp = Secp256k1::new();

        let secret_key = SecretKey::new(&mut OsRng);
        let internal_keypair = Keypair::from_secret_key(&secp, &secret_key);
        let internal_key = internal_keypair.x_only_public_key().0;

        // Two leaves so the merkle path is non-trivial (real branch element).
        let leaf_key = internal_key;
        let leaf0 = checksig_script(&leaf_key);
        let leaf1 = ScriptBuf::from_bytes(vec![0x51]); // OP_TRUE, distinct leaf
        let scripts = vec![leaf0.clone(), leaf1.clone()];

        let taproot_address = manager
            .create_script_path_address(internal_key, scripts.clone())
            .expect("script-path address");
        let spend_info = taproot_address.spend_info.clone();

        let prev_value = Amount::from_sat(50_000_000);
        let prev_output = TxOut {
            value: prev_value,
            script_pubkey: taproot_address.address.script_pubkey(),
        };
        let txid = "0000000000000000000000000000000000000000000000000000000000000002";
        let spend =
            TaprootSpendInput::new(txid, 1, prev_output.clone(), secret_key).expect("spend input");

        let dest_kp = manager.generate_keypair().expect("dest keypair");
        let dest = manager
            .create_key_path_address(dest_kp.internal_key)
            .expect("dest address")
            .address;

        let builder = TaprootTxBuilder::new(manager.clone());
        let fee = Amount::from_sat(2_000);
        let amount = prev_value - fee;
        let tx = builder
            .create_script_path_spend(spend, dest, amount, 0, &spend_info, &scripts)
            .expect("script-path spend");

        // Witness stack must be [signature, script, control_block].
        assert_eq!(tx.input.len(), 1);
        let witness = &tx.input[0].witness;
        assert_eq!(witness.len(), 3, "script-path witness has 3 elements");
        let mut iter = witness.iter();
        let sig_el = iter.next().expect("sig element");
        let script_el = iter.next().expect("script element");
        let cb_el = iter.next().expect("control block element");

        assert_eq!(sig_el.len(), 64, "default-sighash schnorr sig is 64 bytes");
        assert_eq!(
            script_el,
            leaf0.as_bytes(),
            "second item is the spent script"
        );

        // Control block must be well-formed (33 + 32*n bytes) and present a real
        // merkle path (one 32-byte branch element for a 2-leaf tree).
        assert_eq!(
            (cb_el.len() - 33) % 32,
            0,
            "control block size is 33 + 32*path_len"
        );
        assert_eq!(cb_el.len(), 33 + 32, "2-leaf tree => one branch hash");

        // The control block must verify the leaf against the output key (correct
        // merkle path + parity), reconstructing the committed taproot output.
        let control_block =
            bitcoin::taproot::ControlBlock::decode(cb_el).expect("decode control block");
        let output_key = spend_info.output_key().to_x_only_public_key();
        assert!(
            control_block.verify_taproot_commitment(&secp, output_key, &leaf0),
            "control block must prove the leaf commits to the output key"
        );

        // The leaf signature must verify against the leaf key for the tapscript sighash.
        let leaf_hash = TapLeafHash::from_script(&leaf0, LeafVersion::TapScript);
        let prevouts = [prev_output];
        let mut cache = SighashCache::new(&tx);
        let sighash = cache
            .taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&prevouts),
                leaf_hash,
                TapSighashType::Default,
            )
            .expect("recompute tapscript sighash");
        let message = Message::from_digest(sighash.to_byte_array());
        let parsed_sig = SchnorrSignature::from_slice(sig_el).expect("parse schnorr sig");
        secp.verify_schnorr(&parsed_sig, &message, &leaf_key)
            .expect("leaf signature verifies against the leaf key");
    }
}
