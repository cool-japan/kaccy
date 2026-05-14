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
use bitcoin::secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use bitcoin::{Address, Network, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Silent payment address (sp1q... format)
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

    /// Encode as BIP 352 address string (sp1q...)
    pub fn encode(&self) -> String {
        // Simplified encoding - in production this would follow BIP 352 spec
        let prefix = match self.network {
            Network::Bitcoin => "sp1q",
            Network::Testnet => "tsp1q",
            Network::Signet => "ssp1q",
            Network::Regtest => "rsp1q",
            _ => "sp1q",
        };

        format!(
            "{}{}{}",
            prefix,
            hex::encode(self.scan_pubkey.serialize()),
            hex::encode(self.spend_pubkey.serialize())
        )
    }

    /// Parse from BIP 352 address string
    pub fn from_string(s: &str) -> Result<Self, BitcoinError> {
        // Determine network from prefix
        let (network, prefix_len) = if s.starts_with("sp1q") {
            (Network::Bitcoin, 4)
        } else if s.starts_with("tsp1q") {
            (Network::Testnet, 5)
        } else if s.starts_with("ssp1q") {
            (Network::Signet, 5)
        } else if s.starts_with("rsp1q") {
            (Network::Regtest, 5)
        } else {
            return Err(BitcoinError::InvalidAddress(
                "Invalid silent payment address prefix".to_string(),
            ));
        };

        // Extract hex data after prefix
        let hex_data = &s[prefix_len..];

        // BIP 352 address format: prefix + hex(scan_pubkey) + hex(spend_pubkey)
        // Each public key is 33 bytes (compressed)
        if hex_data.len() != 132 {
            // 33 * 2 * 2 = 132 hex chars for two compressed pubkeys
            return Err(BitcoinError::InvalidAddress(format!(
                "Invalid address length: expected 132 hex chars, got {}",
                hex_data.len()
            )));
        }

        // Parse scan public key (first 66 hex chars = 33 bytes)
        let scan_bytes = hex::decode(&hex_data[..66])
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid scan pubkey hex: {}", e)))?;

        let scan_pubkey = PublicKey::from_slice(&scan_bytes)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid scan public key: {}", e)))?;

        // Parse spend public key (next 66 hex chars = 33 bytes)
        let spend_bytes = hex::decode(&hex_data[66..]).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid spend pubkey hex: {}", e))
        })?;

        let spend_pubkey = PublicKey::from_slice(&spend_bytes).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid spend public key: {}", e))
        })?;

        Ok(Self {
            scan_pubkey,
            spend_pubkey,
            label: None, // Labels not encoded in address string
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

        // Apply label if provided (BIP 352 label tweak)
        if let Some(label_value) = label {
            let label_tweak = self.compute_label_tweak(label_value)?;
            spend_pubkey = spend_pubkey
                .add_exp_tweak(&self.secp, &label_tweak)
                .map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to apply label tweak: {}", e))
                })?;
        }

        Ok(SilentPaymentAddress::new(
            scan_pubkey,
            spend_pubkey,
            label,
            self.network,
        ))
    }

    /// Compute label tweak for address organization
    fn compute_label_tweak(&self, label: u32) -> Result<Scalar, BitcoinError> {
        // BIP 352: hash(b_scan || label)
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        let mut engine = sha256::Hash::engine();
        engine.input(&self.scan_privkey.secret_bytes());
        engine.input(&label.to_le_bytes());
        let hash = sha256::Hash::from_engine(engine);

        Scalar::from_be_bytes(hash.to_byte_array())
            .map_err(|_| BitcoinError::InvalidAddress("Failed to compute label tweak".to_string()))
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

    /// Create output for silent payment
    ///
    /// Uses ECDH to derive a unique output public key that only the recipient can spend
    pub fn create_output(
        &self,
        recipient: &SilentPaymentAddress,
        input_privkeys: &[SecretKey],
        output_index: u32,
        amount: u64,
    ) -> Result<TxOut, BitcoinError> {
        // Compute shared secret using ECDH
        let shared_secret =
            self.compute_shared_secret(&recipient.scan_pubkey, input_privkeys, output_index)?;

        // Derive output public key: P_output = B_spend + shared_secret*G
        let output_pubkey = recipient
            .spend_pubkey
            .add_exp_tweak(&self.secp, &shared_secret)
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to derive output key: {}", e))
            })?;

        // Create P2TR output (Taproot key-path spend)
        let address = Address::p2tr(
            &self.secp,
            output_pubkey.x_only_public_key().0,
            None,
            recipient.network,
        );

        Ok(TxOut {
            value: bitcoin::Amount::from_sat(amount),
            script_pubkey: address.script_pubkey(),
        })
    }

    /// Compute shared secret using ECDH
    fn compute_shared_secret(
        &self,
        scan_pubkey: &PublicKey,
        input_privkeys: &[SecretKey],
        output_index: u32,
    ) -> Result<Scalar, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Sum all input private keys
        let mut sum = input_privkeys[0];
        for privkey in &input_privkeys[1..] {
            sum = sum.add_tweak(&(*privkey).into()).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to sum private keys: {}", e))
            })?;
        }

        // Compute ECDH point: sum(input_privkeys) * B_scan
        let ecdh_point = scan_pubkey
            .mul_tweak(&self.secp, &sum.into())
            .map_err(|e| BitcoinError::InvalidAddress(format!("ECDH computation failed: {}", e)))?;

        // Hash the ECDH point with output index
        let mut engine = sha256::Hash::engine();
        engine.input(&ecdh_point.serialize());
        engine.input(&output_index.to_le_bytes());
        let hash = sha256::Hash::from_engine(engine);

        Scalar::from_be_bytes(hash.to_byte_array()).map_err(|_| {
            BitcoinError::InvalidAddress("Failed to compute shared secret".to_string())
        })
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
    #[allow(dead_code)]
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
    /// Derived spend private key for this output
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

    /// Scan a transaction for owned outputs
    pub fn scan_transaction(
        &mut self,
        tx: &bitcoin::Transaction,
    ) -> Result<Vec<DetectedOutput>, BitcoinError> {
        let mut detected = Vec::new();

        // Extract input public keys (simplified - real implementation more complex)
        let input_pubkeys = self.extract_input_pubkeys(tx)?;

        // Check each output
        for (vout, output) in tx.output.iter().enumerate() {
            if let Some(detected_output) =
                self.check_output(tx.compute_txid(), vout as u32, output, &input_pubkeys)?
            {
                detected.push(detected_output);
            }
        }

        // Cache detected outputs
        if !detected.is_empty() {
            self.detected_outputs
                .insert(tx.compute_txid(), detected.clone());
        }

        Ok(detected)
    }

    /// Extract input public keys from transaction
    fn extract_input_pubkeys(
        &self,
        tx: &bitcoin::Transaction,
    ) -> Result<Vec<PublicKey>, BitcoinError> {
        let mut pubkeys = Vec::new();

        // Extract public keys from each input
        for input in &tx.input {
            // Try to extract from witness (P2WPKH, Taproot)
            if !input.witness.is_empty() {
                // P2WPKH witness: [signature, pubkey]
                if input.witness.len() == 2 {
                    let pubkey_bytes = input.witness.nth(1).expect("witness has exactly 2 elements per len == 2 check above");
                    if let Ok(pk) = PublicKey::from_slice(pubkey_bytes) {
                        pubkeys.push(pk);
                        continue;
                    }
                }

                // Taproot witness (key-path spend): [signature]
                // For taproot key-path, we can't directly extract the public key from witness
                // We would need to check the prevout script_pubkey
                // For now, skip taproot inputs in this simplified implementation
            }

            // Try to extract from scriptSig (P2PKH, P2SH)
            let script_sig = &input.script_sig;
            if !script_sig.is_empty() {
                // P2PKH scriptSig: [signature, pubkey]
                // Try to extract the last push which is typically the pubkey
                for instruction in script_sig.instructions() {
                    if let Ok(bitcoin::blockdata::script::Instruction::PushBytes(bytes)) =
                        instruction
                    {
                        // Try to parse as public key
                        if let Ok(pk) = PublicKey::from_slice(bytes.as_bytes()) {
                            pubkeys.push(pk);
                        }
                    }
                }
            }
        }

        Ok(pubkeys)
    }

    /// Check if an output belongs to this wallet
    fn check_output(
        &self,
        txid: bitcoin::Txid,
        vout: u32,
        output: &TxOut,
        input_pubkeys: &[PublicKey],
    ) -> Result<Option<DetectedOutput>, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Silent payments require at least one input public key
        if input_pubkeys.is_empty() {
            return Ok(None);
        }

        // Extract output public key from script_pubkey
        // Silent payments use P2TR outputs
        let output_pk = if output.script_pubkey.is_p2tr() {
            // Extract x-only pubkey from P2TR script
            // P2TR scriptPubkey: OP_1 <32-byte-x-only-pubkey>
            let script_bytes = output.script_pubkey.as_bytes();
            if script_bytes.len() != 34 {
                return Ok(None);
            }

            // Parse x-only public key
            let mut xonly_bytes = [0u8; 32];
            xonly_bytes.copy_from_slice(&script_bytes[2..34]);

            match bitcoin::secp256k1::XOnlyPublicKey::from_slice(&xonly_bytes) {
                Ok(pk) => pk,
                Err(_) => return Ok(None), // Invalid pubkey, not our output
            }
        } else {
            // Not a P2TR output, not a silent payment
            return Ok(None);
        };

        // Compute shared secret: sum(input_pubkeys) * b_scan
        let secp = bitcoin::secp256k1::Secp256k1::new();

        // Sum all input public keys
        let mut sum_pk = input_pubkeys[0];
        for pk in &input_pubkeys[1..] {
            sum_pk = sum_pk.combine(pk).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to sum input pubkeys: {}", e))
            })?;
        }

        // Multiply by scan private key to get ECDH point
        let ecdh_point = sum_pk
            .mul_tweak(&secp, &self.wallet.scan_privkey.into())
            .map_err(|e| BitcoinError::InvalidAddress(format!("ECDH failed: {}", e)))?;

        // Try different labels (including no label)
        let labels_to_try = vec![None, Some(0), Some(1), Some(2), Some(3)]; // Try first few labels

        for label in labels_to_try {
            // Compute shared secret for this output index
            let mut engine = sha256::Hash::engine();
            engine.input(&ecdh_point.serialize());
            engine.input(&vout.to_le_bytes());
            let hash = sha256::Hash::from_engine(engine);

            let shared_secret = bitcoin::secp256k1::Scalar::from_be_bytes(hash.to_byte_array())
                .map_err(|_| {
                    BitcoinError::InvalidAddress("Failed to compute shared secret".to_string())
                })?;

            // Derive expected output pubkey: B_spend + shared_secret*G
            let mut spend_pk =
                bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &self.wallet.spend_privkey);

            // Apply label tweak if needed
            if let Some(label_value) = label {
                let label_tweak = self.wallet.compute_label_tweak(label_value)?;
                spend_pk = spend_pk.add_exp_tweak(&secp, &label_tweak).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to apply label tweak: {}", e))
                })?;
            }

            // Add shared secret to spend pubkey
            let expected_pk = spend_pk.add_exp_tweak(&secp, &shared_secret).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to derive output key: {}", e))
            })?;

            // Compare with actual output pubkey
            if expected_pk.x_only_public_key().0 == output_pk {
                // Found a match! This output belongs to us
                // Derive the spend private key for this output
                let spend_privkey = self
                    .wallet
                    .spend_privkey
                    .add_tweak(&shared_secret)
                    .map_err(|e| {
                        BitcoinError::InvalidAddress(format!(
                            "Failed to derive spend privkey: {}",
                            e
                        ))
                    })?;

                return Ok(Some(DetectedOutput {
                    txid,
                    vout,
                    amount: output.value.to_sat(),
                    spend_privkey,
                    label,
                }));
            }
        }

        // No match found
        Ok(None)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_sender_create_output() {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let wallet = SilentPaymentWallet::new().unwrap();
        let address = wallet.get_address(None).unwrap();
        let sender = SilentPaymentSender::new();

        let input_key = SecretKey::new(&mut OsRng);

        let output = sender
            .create_output(&address, &[input_key], 0, 100_000)
            .unwrap();

        assert_eq!(output.value.to_sat(), 100_000);
        assert!(!output.script_pubkey.is_empty());
    }

    #[test]
    fn test_scanner_creation() {
        let wallet = SilentPaymentWallet::new().unwrap();
        let scanner = SilentPaymentScanner::new(wallet);

        assert_eq!(scanner.get_balance(), 0);
        assert!(scanner.get_detected_outputs().is_empty());
    }

    #[test]
    fn test_address_to_string() {
        let wallet = SilentPaymentWallet::new_with_network(Network::Testnet).unwrap();
        let address = wallet.get_address(None).unwrap();
        let addr_string = address.encode();

        assert!(addr_string.starts_with("tsp1q"));
        assert!(addr_string.len() > 10);
    }

    #[test]
    fn test_address_round_trip() {
        let wallet = SilentPaymentWallet::new_with_network(Network::Bitcoin).unwrap();
        let address = wallet.get_address(None).unwrap();
        let addr_string = address.encode();

        let parsed_address = SilentPaymentAddress::from_string(&addr_string).unwrap();

        assert_eq!(parsed_address.scan_pubkey, address.scan_pubkey);
        assert_eq!(parsed_address.spend_pubkey, address.spend_pubkey);
        assert_eq!(parsed_address.network, address.network);
    }

    #[test]
    fn test_invalid_address_prefix() {
        let result = SilentPaymentAddress::from_string("invalid123");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_address_length() {
        let result = SilentPaymentAddress::from_string("sp1q12345");
        assert!(result.is_err());
    }
}
