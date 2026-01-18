//! Multi-signature wallet support module
//!
//! This module provides multi-signature wallet capabilities for enhanced
//! security, including 2-of-3, 3-of-5, and other M-of-N configurations.

use bitcoin::{
    Address, Network, ScriptBuf,
    bip32::{DerivationPath, Xpub},
    script::Builder as ScriptBuilder,
    secp256k1::{PublicKey, Secp256k1},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use crate::error::{BitcoinError, Result};

/// Multi-signature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigConfig {
    /// Number of required signatures (M)
    pub required_signatures: u8,
    /// Total number of keys (N)
    pub total_keys: u8,
    /// Extended public keys for all participants
    pub xpubs: Vec<String>,
    /// Key labels/identifiers
    pub key_labels: Vec<String>,
    /// Derivation path for addresses
    pub derivation_path: String,
    /// Network (mainnet, testnet, signet)
    pub network: Network,
}

impl MultisigConfig {
    /// Create a new 2-of-3 multisig configuration
    pub fn new_2of3(xpubs: Vec<String>, key_labels: Vec<String>, network: Network) -> Result<Self> {
        if xpubs.len() != 3 {
            return Err(BitcoinError::Wallet(
                "2-of-3 requires exactly 3 xpubs".to_string(),
            ));
        }
        if key_labels.len() != 3 {
            return Err(BitcoinError::Wallet(
                "2-of-3 requires exactly 3 labels".to_string(),
            ));
        }

        Ok(Self {
            required_signatures: 2,
            total_keys: 3,
            xpubs,
            key_labels,
            derivation_path: "m/48'/0'/0'/2'".to_string(), // BIP48 for multisig
            network,
        })
    }

    /// Create a custom M-of-N configuration
    pub fn new_custom(
        required_signatures: u8,
        xpubs: Vec<String>,
        key_labels: Vec<String>,
        network: Network,
    ) -> Result<Self> {
        let total_keys = xpubs.len() as u8;

        if required_signatures > total_keys {
            return Err(BitcoinError::Wallet(
                "Required signatures cannot exceed total keys".to_string(),
            ));
        }

        if required_signatures == 0 {
            return Err(BitcoinError::Wallet(
                "Required signatures must be at least 1".to_string(),
            ));
        }

        if total_keys > 15 {
            return Err(BitcoinError::Wallet(
                "Maximum 15 keys supported for standard multisig".to_string(),
            ));
        }

        if key_labels.len() != xpubs.len() {
            return Err(BitcoinError::Wallet(
                "Number of labels must match number of xpubs".to_string(),
            ));
        }

        Ok(Self {
            required_signatures,
            total_keys,
            xpubs,
            key_labels,
            derivation_path: "m/48'/0'/0'/2'".to_string(),
            network,
        })
    }

    /// Validate all xpubs
    pub fn validate(&self) -> Result<()> {
        for (i, xpub) in self.xpubs.iter().enumerate() {
            Xpub::from_str(xpub).map_err(|e| {
                BitcoinError::InvalidXpub(format!("Invalid xpub at index {}: {}", i, e))
            })?;
        }
        Ok(())
    }

    /// Get the multisig type string (e.g., "2-of-3")
    pub fn type_string(&self) -> String {
        format!("{}-of-{}", self.required_signatures, self.total_keys)
    }
}

/// Multi-signature wallet
pub struct MultisigWallet {
    config: MultisigConfig,
    /// Parsed extended public keys
    xpubs: Vec<Xpub>,
    /// Address cache
    address_cache: HashMap<u32, MultisigAddress>,
    /// Next address index
    next_index: u32,
}

impl MultisigWallet {
    /// Create a new multisig wallet
    pub fn new(config: MultisigConfig) -> Result<Self> {
        config.validate()?;

        let xpubs: Vec<Xpub> = config
            .xpubs
            .iter()
            .map(|x| Xpub::from_str(x))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| BitcoinError::InvalidXpub(e.to_string()))?;

        Ok(Self {
            config,
            xpubs,
            address_cache: HashMap::new(),
            next_index: 0,
        })
    }

    /// Get the multisig configuration
    pub fn config(&self) -> &MultisigConfig {
        &self.config
    }

    /// Generate a new receiving address
    pub fn get_new_address(&mut self) -> Result<MultisigAddress> {
        let index = self.next_index;
        let address = self.derive_address(index, false)?;
        self.address_cache.insert(index, address.clone());
        self.next_index += 1;
        Ok(address)
    }

    /// Get address at specific index
    pub fn get_address(&mut self, index: u32) -> Result<MultisigAddress> {
        if let Some(cached) = self.address_cache.get(&index) {
            return Ok(cached.clone());
        }

        let address = self.derive_address(index, false)?;
        self.address_cache.insert(index, address.clone());
        Ok(address)
    }

    /// Derive a multisig address
    fn derive_address(&self, index: u32, is_change: bool) -> Result<MultisigAddress> {
        let secp = Secp256k1::new();

        // Derive public keys for this index
        let chain = if is_change { 1 } else { 0 };
        let path = DerivationPath::from_str(&format!("m/{}/{}", chain, index))
            .map_err(|e| BitcoinError::DerivationFailed(e.to_string()))?;

        let mut pubkeys: Vec<PublicKey> = Vec::new();

        for xpub in &self.xpubs {
            let derived = xpub
                .derive_pub(&secp, &path)
                .map_err(|e| BitcoinError::DerivationFailed(e.to_string()))?;
            pubkeys.push(derived.public_key);
        }

        // Sort pubkeys lexicographically for deterministic ordering
        pubkeys.sort_by_key(|a| a.serialize());

        // Create the redeem script
        let redeem_script =
            Self::create_multisig_script(self.config.required_signatures, &pubkeys)?;

        // For P2WSH (native SegWit multisig)
        let witness_script = redeem_script.clone();
        let _script_hash = witness_script.wscript_hash();

        let address = Address::p2wsh(&witness_script, self.config.network);

        Ok(MultisigAddress {
            address: address.to_string(),
            index,
            is_change,
            redeem_script: hex::encode(redeem_script.as_bytes()),
            witness_script: hex::encode(witness_script.as_bytes()),
            pubkeys: pubkeys.iter().map(|p| hex::encode(p.serialize())).collect(),
        })
    }

    /// Create a multisig script
    fn create_multisig_script(required: u8, pubkeys: &[PublicKey]) -> Result<ScriptBuf> {
        let mut builder = ScriptBuilder::new().push_int(required as i64);

        for pubkey in pubkeys {
            let serialized = pubkey.serialize();
            builder = builder.push_slice(serialized);
        }

        let script = builder
            .push_int(pubkeys.len() as i64)
            .push_opcode(bitcoin::opcodes::all::OP_CHECKMULTISIG)
            .into_script();

        Ok(script)
    }

    /// Check if an address belongs to this wallet
    pub fn is_our_address(&self, address: &str) -> bool {
        // Check cache first
        for cached in self.address_cache.values() {
            if cached.address == address {
                return true;
            }
        }

        // Check first 1000 addresses (gap limit)
        for i in 0..1000 {
            if let Ok(addr) = self.derive_address_uncached(i, false) {
                if addr.address == address {
                    return true;
                }
            }
            if let Ok(addr) = self.derive_address_uncached(i, true) {
                if addr.address == address {
                    return true;
                }
            }
        }

        false
    }

    /// Derive address without caching
    fn derive_address_uncached(&self, index: u32, is_change: bool) -> Result<MultisigAddress> {
        self.derive_address(index, is_change)
    }

    /// Get next address index
    pub fn next_index(&self) -> u32 {
        self.next_index
    }

    /// Set next address index (for recovery)
    pub fn set_next_index(&mut self, index: u32) {
        self.next_index = index;
    }
}

/// Multisig address with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigAddress {
    /// Bitcoin address string
    pub address: String,
    /// Derivation index
    pub index: u32,
    /// Whether this is a change address
    pub is_change: bool,
    /// Hex-encoded redeem script
    pub redeem_script: String,
    /// Hex-encoded witness script (for P2WSH)
    pub witness_script: String,
    /// Hex-encoded public keys (sorted)
    pub pubkeys: Vec<String>,
}

/// Partially signed multisig transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigTransaction {
    /// Transaction ID (once finalized)
    pub txid: Option<String>,
    /// Unsigned transaction (hex)
    pub unsigned_tx: String,
    /// PSBT (base64)
    pub psbt: String,
    /// Signatures collected so far
    pub signatures: Vec<MultisigSignature>,
    /// Required number of signatures
    pub required_signatures: u8,
    /// Total signers
    pub total_signers: u8,
    /// Transaction status
    pub status: MultisigTxStatus,
    /// Inputs being spent
    pub inputs: Vec<MultisigInput>,
    /// Outputs
    pub outputs: Vec<MultisigOutput>,
}

impl MultisigTransaction {
    /// Check if transaction has enough signatures
    pub fn has_enough_signatures(&self) -> bool {
        self.signatures.len() as u8 >= self.required_signatures
    }

    /// Get remaining signatures needed
    pub fn signatures_needed(&self) -> u8 {
        self.required_signatures
            .saturating_sub(self.signatures.len() as u8)
    }

    /// Get list of signers who have signed
    pub fn signed_by(&self) -> Vec<&str> {
        self.signatures
            .iter()
            .map(|s| s.signer_label.as_str())
            .collect()
    }

    /// Get list of signers who haven't signed yet
    pub fn pending_signers(&self, all_labels: &[String]) -> Vec<String> {
        let signed: std::collections::HashSet<_> =
            self.signatures.iter().map(|s| &s.signer_label).collect();
        all_labels
            .iter()
            .filter(|l| !signed.contains(*l))
            .cloned()
            .collect()
    }
}

/// A signature for a multisig transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigSignature {
    /// Signer's key label
    pub signer_label: String,
    /// Signer's public key (hex)
    pub signer_pubkey: String,
    /// Signature data (hex)
    pub signature: String,
    /// Input index this signature is for
    pub input_index: u32,
    /// Timestamp when signed
    pub signed_at: String,
}

/// Status of a multisig transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultisigTxStatus {
    /// Awaiting signatures
    Pending,
    /// Has enough signatures, ready to broadcast
    ReadyToBroadcast,
    /// Broadcasted, awaiting confirmation
    Broadcasted,
    /// Confirmed
    Confirmed,
    /// Rejected/cancelled
    Rejected,
}

/// Input for multisig transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigInput {
    /// Previous transaction ID
    pub txid: String,
    /// Previous output index
    pub vout: u32,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Address being spent from
    pub address: String,
    /// Witness script (for signing)
    pub witness_script: String,
}

/// Output for multisig transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigOutput {
    /// Destination address
    pub address: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Whether this is change
    pub is_change: bool,
}

/// Multisig transaction builder
pub struct MultisigTxBuilder {
    wallet: MultisigWallet,
    inputs: Vec<MultisigInput>,
    outputs: Vec<MultisigOutput>,
    fee_rate: u64,
}

impl MultisigTxBuilder {
    /// Create a new transaction builder
    pub fn new(wallet: MultisigWallet) -> Self {
        Self {
            wallet,
            inputs: Vec::new(),
            outputs: Vec::new(),
            fee_rate: 1, // 1 sat/vbyte default
        }
    }

    /// Add an input
    pub fn add_input(mut self, input: MultisigInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Add multiple inputs
    pub fn add_inputs(mut self, inputs: Vec<MultisigInput>) -> Self {
        self.inputs.extend(inputs);
        self
    }

    /// Add an output
    pub fn add_output(mut self, address: impl Into<String>, amount_sats: u64) -> Self {
        self.outputs.push(MultisigOutput {
            address: address.into(),
            amount_sats,
            is_change: false,
        });
        self
    }

    /// Set fee rate (sat/vbyte)
    pub fn fee_rate(mut self, rate: u64) -> Self {
        self.fee_rate = rate;
        self
    }

    /// Calculate estimated transaction size
    fn estimate_vsize(&self) -> u64 {
        // P2WSH multisig size estimation
        // Input: ~(73*m + 34*n + 20) witness bytes + 41 non-witness bytes
        // Output: ~43 bytes for P2WPKH, ~43 for P2WSH
        let m = self.wallet.config.required_signatures as u64;
        let n = self.wallet.config.total_keys as u64;

        let input_weight = self.inputs.len() as u64 * (41 * 4 + 73 * m + 34 * n + 20);
        let output_weight = self.outputs.len() as u64 * 43 * 4;
        let overhead_weight = 44; // 10 bytes * 4 + 4 for marker/flag

        (input_weight + output_weight + overhead_weight).div_ceil(4)
    }

    /// Calculate required fee
    pub fn calculate_fee(&self) -> u64 {
        self.estimate_vsize() * self.fee_rate
    }

    /// Build the unsigned transaction
    pub fn build(mut self) -> Result<MultisigTransaction> {
        if self.inputs.is_empty() {
            return Err(BitcoinError::Wallet("No inputs provided".to_string()));
        }

        if self.outputs.is_empty() {
            return Err(BitcoinError::Wallet("No outputs provided".to_string()));
        }

        let fee = self.calculate_fee();
        let total_input: u64 = self.inputs.iter().map(|i| i.amount_sats).sum();
        let total_output: u64 = self.outputs.iter().map(|o| o.amount_sats).sum();

        if total_input < total_output + fee {
            return Err(BitcoinError::Wallet(format!(
                "Insufficient funds: {} < {} + {} (fee)",
                total_input, total_output, fee
            )));
        }

        // Add change output if needed
        let change = total_input - total_output - fee;
        if change > 546 {
            // Dust threshold
            let change_address = self.wallet.derive_address(self.wallet.next_index, true)?;
            self.outputs.push(MultisigOutput {
                address: change_address.address,
                amount_sats: change,
                is_change: true,
            });
        }

        // In production, this would create actual PSBT
        // For now, return a placeholder structure
        let unsigned_tx = format!(
            "unsigned_tx_{}inputs_{}outputs",
            self.inputs.len(),
            self.outputs.len()
        );

        let psbt = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("psbt_placeholder_{}", unsigned_tx).as_bytes(),
        );

        Ok(MultisigTransaction {
            txid: None,
            unsigned_tx,
            psbt,
            signatures: Vec::new(),
            required_signatures: self.wallet.config.required_signatures,
            total_signers: self.wallet.config.total_keys,
            status: MultisigTxStatus::Pending,
            inputs: self.inputs,
            outputs: self.outputs,
        })
    }
}

/// Multisig custody manager for large balance handling
pub struct CustodyManager {
    /// Hot wallet (single-sig for quick operations)
    #[allow(dead_code)]
    hot_wallet_xpub: Option<String>,
    /// Cold storage (multisig)
    cold_wallet: Option<MultisigWallet>,
    /// Threshold for auto-sweep to cold storage
    auto_sweep_threshold_sats: u64,
    /// Minimum hot wallet balance to maintain
    min_hot_balance_sats: u64,
}

impl CustodyManager {
    /// Create a new custody manager
    pub fn new() -> Self {
        Self {
            hot_wallet_xpub: None,
            cold_wallet: None,
            auto_sweep_threshold_sats: 10_000_000, // 0.1 BTC
            min_hot_balance_sats: 1_000_000,       // 0.01 BTC
        }
    }

    /// Set up hot wallet
    pub fn with_hot_wallet(mut self, xpub: impl Into<String>) -> Self {
        self.hot_wallet_xpub = Some(xpub.into());
        self
    }

    /// Set up cold storage
    pub fn with_cold_wallet(mut self, config: MultisigConfig) -> Result<Self> {
        self.cold_wallet = Some(MultisigWallet::new(config)?);
        Ok(self)
    }

    /// Set auto-sweep threshold
    pub fn auto_sweep_threshold(mut self, sats: u64) -> Self {
        self.auto_sweep_threshold_sats = sats;
        self
    }

    /// Set minimum hot balance
    pub fn min_hot_balance(mut self, sats: u64) -> Self {
        self.min_hot_balance_sats = sats;
        self
    }

    /// Check if balance should be swept to cold storage
    pub fn should_sweep(&self, hot_balance_sats: u64) -> bool {
        hot_balance_sats > self.auto_sweep_threshold_sats
    }

    /// Calculate sweep amount
    pub fn sweep_amount(&self, hot_balance_sats: u64) -> u64 {
        if hot_balance_sats <= self.auto_sweep_threshold_sats {
            return 0;
        }

        // Keep minimum in hot wallet, sweep the rest
        hot_balance_sats.saturating_sub(self.min_hot_balance_sats)
    }

    /// Get cold storage address for sweeping
    pub fn get_cold_address(&mut self) -> Result<String> {
        let wallet = self
            .cold_wallet
            .as_mut()
            .ok_or_else(|| BitcoinError::Wallet("Cold wallet not configured".to_string()))?;

        let address = wallet.get_new_address()?;
        Ok(address.address)
    }

    /// Check if cold wallet is configured
    pub fn has_cold_storage(&self) -> bool {
        self.cold_wallet.is_some()
    }

    /// Get cold wallet info
    pub fn cold_wallet_info(&self) -> Option<ColdWalletInfo> {
        self.cold_wallet.as_ref().map(|w| ColdWalletInfo {
            type_string: w.config.type_string(),
            key_labels: w.config.key_labels.clone(),
            next_address_index: w.next_index,
        })
    }
}

impl Default for CustodyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Cold wallet information summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdWalletInfo {
    /// Multisig type (e.g., "2-of-3")
    pub type_string: String,
    /// Key holder labels
    pub key_labels: Vec<String>,
    /// Next address index
    pub next_address_index: u32,
}

/// Type alias for transaction ready callback
type TransactionReadyCallback = Box<dyn Fn(&MultisigTransaction) + Send + Sync>;

/// Signature coordinator for collecting multisig signatures
pub struct SignatureCoordinator {
    /// Pending transactions awaiting signatures
    pending_txs: HashMap<String, MultisigTransaction>,
    /// Notification callbacks
    #[allow(dead_code)]
    on_ready: Option<TransactionReadyCallback>,
}

impl SignatureCoordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        Self {
            pending_txs: HashMap::new(),
            on_ready: None,
        }
    }

    /// Set callback for when transaction is ready to broadcast
    pub fn on_ready<F>(mut self, callback: F) -> Self
    where
        F: Fn(&MultisigTransaction) + Send + Sync + 'static,
    {
        self.on_ready = Some(Box::new(callback));
        self
    }

    /// Add a transaction for signature collection
    pub fn add_transaction(&mut self, id: impl Into<String>, tx: MultisigTransaction) {
        self.pending_txs.insert(id.into(), tx);
    }

    /// Add a signature to a pending transaction
    pub fn add_signature(&mut self, tx_id: &str, signature: MultisigSignature) -> Result<bool> {
        let tx = self
            .pending_txs
            .get_mut(tx_id)
            .ok_or_else(|| BitcoinError::Wallet(format!("Transaction {} not found", tx_id)))?;

        // Check if already signed by this signer
        if tx
            .signatures
            .iter()
            .any(|s| s.signer_label == signature.signer_label)
        {
            return Err(BitcoinError::Wallet(format!(
                "Already signed by {}",
                signature.signer_label
            )));
        }

        tx.signatures.push(signature);

        // Check if ready
        if tx.has_enough_signatures() {
            tx.status = MultisigTxStatus::ReadyToBroadcast;

            if let Some(ref callback) = self.on_ready {
                callback(tx);
            }

            return Ok(true);
        }

        Ok(false)
    }

    /// Get pending transaction status
    pub fn get_status(&self, tx_id: &str) -> Option<&MultisigTransaction> {
        self.pending_txs.get(tx_id)
    }

    /// Get all pending transactions
    pub fn pending_transactions(&self) -> Vec<(&String, &MultisigTransaction)> {
        self.pending_txs.iter().collect()
    }

    /// Remove a transaction (after broadcast or cancellation)
    pub fn remove_transaction(&mut self, tx_id: &str) -> Option<MultisigTransaction> {
        self.pending_txs.remove(tx_id)
    }
}

impl Default for SignatureCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multisig_config_validation() {
        // Valid 2-of-3
        let config = MultisigConfig::new_2of3(
            vec![
                "xpub1".to_string(),
                "xpub2".to_string(),
                "xpub3".to_string(),
            ],
            vec![
                "platform".to_string(),
                "user".to_string(),
                "cold".to_string(),
            ],
            Network::Bitcoin,
        )
        .unwrap();

        assert_eq!(config.type_string(), "2-of-3");
        assert_eq!(config.required_signatures, 2);
        assert_eq!(config.total_keys, 3);
    }

    #[test]
    fn test_invalid_config() {
        // Wrong number of xpubs
        let result = MultisigConfig::new_2of3(
            vec!["xpub1".to_string(), "xpub2".to_string()],
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            Network::Bitcoin,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_multisig_tx_signatures() {
        let tx = MultisigTransaction {
            txid: None,
            unsigned_tx: "test".to_string(),
            psbt: "test".to_string(),
            signatures: vec![],
            required_signatures: 2,
            total_signers: 3,
            status: MultisigTxStatus::Pending,
            inputs: vec![],
            outputs: vec![],
        };

        assert!(!tx.has_enough_signatures());
        assert_eq!(tx.signatures_needed(), 2);
    }

    #[test]
    fn test_custody_manager() {
        let manager = CustodyManager::new()
            .auto_sweep_threshold(10_000_000)
            .min_hot_balance(1_000_000);

        assert!(manager.should_sweep(15_000_000));
        assert!(!manager.should_sweep(5_000_000));

        let sweep = manager.sweep_amount(15_000_000);
        assert_eq!(sweep, 14_000_000); // 15M - 1M min balance
    }
}
