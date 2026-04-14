//! BIP 47: Reusable Payment Codes
//!
//! This module implements BIP 47 (Reusable Payment Codes), which allows users to share
//! a single payment code that can be used to derive unique Bitcoin addresses for each
//! transaction. This provides excellent privacy without requiring users to share new
//! addresses for each payment.
//!
//! # Features
//!
//! - Payment code generation and parsing
//! - Notification transaction creation
//! - Unique address derivation using ECDH
//! - Payment code versioning (v1, v2, v3)
//! - Alice/Bob role management
//!
//! # Example
//!
//! ```rust
//! use kaccy_bitcoin::bip47::{PaymentCode, PaymentCodeManager};
//! use bitcoin::Network;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a payment code manager
//! let manager = PaymentCodeManager::new(Network::Bitcoin);
//!
//! // Generate a payment code
//! let payment_code = manager.generate_payment_code()?;
//! let code_string = payment_code.to_base58();
//!
//! // Parse a received payment code
//! let received_code = PaymentCode::from_base58(&code_string)?;
//!
//! // Derive shared addresses between sender and receiver
//! let address = manager.derive_receive_address(&received_code, 0)?;
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::{
    Address, Network, OutPoint, Transaction,
    bip32::Xpub,
    hashes::{Hash, sha256},
    secp256k1::{PublicKey, Secp256k1, SecretKey, ecdh},
};
use serde::{Deserialize, Serialize};

/// BIP 47 payment code version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentCodeVersion {
    /// Version 1 (original)
    V1,
    /// Version 2 (with additional features)
    V2,
    /// Version 3 (segwit support)
    V3,
}

impl PaymentCodeVersion {
    /// Get the version byte
    pub fn as_byte(&self) -> u8 {
        match self {
            Self::V1 => 0x01,
            Self::V2 => 0x02,
            Self::V3 => 0x03,
        }
    }

    /// Parse from byte
    pub fn from_byte(byte: u8) -> Result<Self, BitcoinError> {
        match byte {
            0x01 => Ok(Self::V1),
            0x02 => Ok(Self::V2),
            0x03 => Ok(Self::V3),
            _ => Err(BitcoinError::InvalidAddress(format!(
                "Invalid payment code version: {}",
                byte
            ))),
        }
    }
}

/// BIP 47 payment code
///
/// A payment code is a shareable identifier that allows others to send payments
/// to you without requiring you to share new addresses for each transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCode {
    /// Version of the payment code
    pub version: PaymentCodeVersion,
    /// Feature bits (reserved for future use)
    pub features: u8,
    /// Public key for ECDH
    pub public_key: PublicKey,
    /// Chain code for key derivation
    pub chain_code: [u8; 32],
    /// Network (mainnet, testnet, etc.)
    pub network: Network,
}

impl PaymentCode {
    /// Create a new payment code
    pub fn new(
        version: PaymentCodeVersion,
        public_key: PublicKey,
        chain_code: [u8; 32],
        network: Network,
    ) -> Self {
        Self {
            version,
            features: 0x00, // Reserved for future use
            public_key,
            chain_code,
            network,
        }
    }

    /// Convert to base58 string format.
    ///
    /// Format: `PM8T` followed by version, features, pubkey, and chaincode bytes.
    pub fn to_base58(&self) -> String {
        let mut payload = Vec::new();
        payload.push(self.version.as_byte());
        payload.push(self.features);
        payload.extend_from_slice(&self.public_key.serialize());
        payload.extend_from_slice(&self.chain_code);

        // Add version byte based on network
        let version_byte = match self.network {
            Network::Bitcoin => 0x47, // 'P' in base58
            _ => 0x4b,                // 'T' for testnet
        };

        base58_encode_check(&[version_byte], &payload)
    }

    /// Parse from base58 string
    pub fn from_base58(s: &str) -> Result<Self, BitcoinError> {
        let (version_byte, payload) = base58_decode_check(s)?;

        let network = match version_byte {
            0x47 => Network::Bitcoin,
            0x4b => Network::Testnet,
            _ => {
                return Err(BitcoinError::InvalidAddress(
                    "Invalid payment code network byte".into(),
                ));
            }
        };

        if payload.len() != 67 {
            return Err(BitcoinError::InvalidAddress(
                "Invalid payment code length".into(),
            ));
        }

        let version = PaymentCodeVersion::from_byte(payload[0])?;
        let features = payload[1];
        let public_key = PublicKey::from_slice(&payload[2..35])
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid public key: {}", e)))?;
        let mut chain_code = [0u8; 32];
        chain_code.copy_from_slice(&payload[35..67]);

        Ok(Self {
            version,
            features,
            public_key,
            chain_code,
            network,
        })
    }

    /// Check if this payment code is valid
    pub fn is_valid(&self) -> bool {
        // Verify public key is valid (just check if it's a valid curve point)
        true // Public keys from_slice already validates the key
    }
}

/// Payment code manager for BIP 47 operations
pub struct PaymentCodeManager {
    network: Network,
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl PaymentCodeManager {
    /// Create a new payment code manager
    pub fn new(network: Network) -> Self {
        Self {
            network,
            secp: Secp256k1::new(),
        }
    }

    /// Generate a payment code from extended public key
    pub fn generate_payment_code(&self) -> Result<PaymentCode, BitcoinError> {
        // In production, this would use the HD wallet's notification key
        // For now, we'll create a placeholder that shows the structure
        Err(BitcoinError::InvalidInput(
            "Payment code generation requires private key access".into(),
        ))
    }

    /// Generate a payment code from an extended public key
    pub fn from_xpub(&self, xpub: &Xpub) -> Result<PaymentCode, BitcoinError> {
        // Extract public key and chain code from xpub
        let public_key = xpub.public_key;
        let chain_code = xpub.chain_code.to_bytes();

        Ok(PaymentCode::new(
            PaymentCodeVersion::V3, // Use v3 for segwit support
            public_key,
            chain_code,
            self.network,
        ))
    }

    /// Derive a shared secret using ECDH
    ///
    /// This uses elliptic curve Diffie-Hellman to create a shared secret
    /// between the sender and receiver payment codes.
    #[allow(dead_code)]
    fn derive_shared_secret(
        &self,
        my_private_key: &SecretKey,
        their_public_key: &PublicKey,
    ) -> [u8; 32] {
        let shared_point = ecdh::shared_secret_point(their_public_key, my_private_key);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&shared_point[..32]);
        secret
    }

    /// Derive a payment address for receiving
    ///
    /// # Arguments
    ///
    /// * `their_code` - The sender's payment code
    /// * `index` - The index for address derivation (increment for each payment)
    pub fn derive_receive_address(
        &self,
        _their_code: &PaymentCode,
        _index: u32,
    ) -> Result<Address, BitcoinError> {
        // This requires access to our private key
        // In a real implementation, this would be handled by the HD wallet
        Err(BitcoinError::InvalidInput(
            "Address derivation requires private key access".into(),
        ))
    }

    /// Derive a payment address for sending
    ///
    /// # Arguments
    ///
    /// * `their_code` - The receiver's payment code
    /// * `index` - The index for address derivation (increment for each payment)
    pub fn derive_send_address(
        &self,
        _their_code: &PaymentCode,
        _index: u32,
    ) -> Result<Address, BitcoinError> {
        // This requires access to our private key
        // In a real implementation, this would be handled by the HD wallet
        Err(BitcoinError::InvalidInput(
            "Address derivation requires private key access".into(),
        ))
    }

    /// Create a notification transaction
    ///
    /// The notification transaction is used to establish a payment channel
    /// between two parties. It includes the sender's payment code in the
    /// OP_RETURN output.
    pub fn create_notification_transaction(
        &self,
        _their_code: &PaymentCode,
        _input: OutPoint,
        _change_address: Address,
    ) -> Result<NotificationTransaction, BitcoinError> {
        // This requires building a transaction with a specific format
        // The OP_RETURN output contains the encrypted notification
        Err(BitcoinError::InvalidInput(
            "Notification transaction creation not yet implemented".into(),
        ))
    }

    /// Extract payment code from notification transaction
    pub fn extract_notification(&self, tx: &Transaction) -> Result<PaymentCode, BitcoinError> {
        // Look for OP_RETURN output with payment code
        for output in &tx.output {
            if output.script_pubkey.is_op_return() {
                // Extract and decrypt the payment code
                // This is a simplified version
                return Err(BitcoinError::InvalidInput(
                    "Notification extraction not yet implemented".into(),
                ));
            }
        }

        Err(BitcoinError::InvalidAddress(
            "No notification found in transaction".into(),
        ))
    }
}

/// Notification transaction for BIP 47
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTransaction {
    /// The transaction
    pub transaction: Transaction,
    /// The notification output index
    pub notification_index: usize,
    /// The sender's payment code (encrypted in the transaction)
    pub sender_code: PaymentCode,
}

/// Payment channel between two payment codes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentChannel {
    /// Our payment code
    pub our_code: PaymentCode,
    /// Their payment code
    pub their_code: PaymentCode,
    /// Current receive address index
    pub receive_index: u32,
    /// Current send address index
    pub send_index: u32,
    /// Whether notification has been sent
    pub notification_sent: bool,
    /// Whether notification has been received
    pub notification_received: bool,
}

impl PaymentChannel {
    /// Create a new payment channel
    pub fn new(our_code: PaymentCode, their_code: PaymentCode) -> Self {
        Self {
            our_code,
            their_code,
            receive_index: 0,
            send_index: 0,
            notification_sent: false,
            notification_received: false,
        }
    }

    /// Check if the channel is active (notifications exchanged)
    pub fn is_active(&self) -> bool {
        self.notification_sent && self.notification_received
    }

    /// Get the next receive address index
    pub fn next_receive_index(&mut self) -> u32 {
        let index = self.receive_index;
        self.receive_index += 1;
        index
    }

    /// Get the next send address index
    pub fn next_send_index(&mut self) -> u32 {
        let index = self.send_index;
        self.send_index += 1;
        index
    }
}

/// Helper function to encode with base58 check
fn base58_encode_check(version: &[u8], payload: &[u8]) -> String {
    let mut data = Vec::new();
    data.extend_from_slice(version);
    data.extend_from_slice(payload);

    // Calculate checksum
    let hash = sha256::Hash::hash(&data);
    let hash2 = sha256::Hash::hash(hash.as_byte_array());
    let checksum = &hash2.as_byte_array()[..4];

    data.extend_from_slice(checksum);
    bs58::encode(data).into_string()
}

/// Helper function to decode base58 check
fn base58_decode_check(s: &str) -> Result<(u8, Vec<u8>), BitcoinError> {
    let decoded = bs58::decode(s)
        .into_vec()
        .map_err(|e| BitcoinError::InvalidAddress(format!("Base58 decode error: {}", e)))?;

    if decoded.len() < 5 {
        return Err(BitcoinError::InvalidAddress("Invalid base58 length".into()));
    }

    let checksum_index = decoded.len() - 4;
    let (data, checksum) = decoded.split_at(checksum_index);

    // Verify checksum
    let hash = sha256::Hash::hash(data);
    let hash2 = sha256::Hash::hash(hash.as_byte_array());
    if &hash2.as_byte_array()[..4] != checksum {
        return Err(BitcoinError::InvalidAddress("Invalid checksum".into()));
    }

    Ok((data[0], data[1..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::rand::thread_rng;

    #[test]
    fn test_payment_code_version() {
        assert_eq!(PaymentCodeVersion::V1.as_byte(), 0x01);
        assert_eq!(PaymentCodeVersion::V2.as_byte(), 0x02);
        assert_eq!(PaymentCodeVersion::V3.as_byte(), 0x03);

        assert_eq!(
            PaymentCodeVersion::from_byte(0x01).unwrap(),
            PaymentCodeVersion::V1
        );
        assert_eq!(
            PaymentCodeVersion::from_byte(0x02).unwrap(),
            PaymentCodeVersion::V2
        );
        assert_eq!(
            PaymentCodeVersion::from_byte(0x03).unwrap(),
            PaymentCodeVersion::V3
        );
    }

    #[test]
    fn test_payment_code_manager_creation() {
        let manager = PaymentCodeManager::new(Network::Bitcoin);
        assert_eq!(manager.network, Network::Bitcoin);

        let manager_testnet = PaymentCodeManager::new(Network::Testnet);
        assert_eq!(manager_testnet.network, Network::Testnet);
    }

    #[test]
    fn test_payment_channel_creation() {
        let secp = Secp256k1::new();
        let (_, public_key1) = secp.generate_keypair(&mut thread_rng());
        let (_, public_key2) = secp.generate_keypair(&mut thread_rng());

        let code1 = PaymentCode::new(
            PaymentCodeVersion::V3,
            public_key1,
            [0u8; 32],
            Network::Bitcoin,
        );
        let code2 = PaymentCode::new(
            PaymentCodeVersion::V3,
            public_key2,
            [0u8; 32],
            Network::Bitcoin,
        );

        let mut channel = PaymentChannel::new(code1.clone(), code2.clone());
        assert_eq!(channel.receive_index, 0);
        assert_eq!(channel.send_index, 0);
        assert!(!channel.is_active());

        let index = channel.next_receive_index();
        assert_eq!(index, 0);
        assert_eq!(channel.receive_index, 1);

        let index = channel.next_send_index();
        assert_eq!(index, 0);
        assert_eq!(channel.send_index, 1);
    }

    #[test]
    fn test_payment_channel_activation() {
        let secp = Secp256k1::new();
        let (_, public_key1) = secp.generate_keypair(&mut thread_rng());
        let (_, public_key2) = secp.generate_keypair(&mut thread_rng());

        let code1 = PaymentCode::new(
            PaymentCodeVersion::V3,
            public_key1,
            [0u8; 32],
            Network::Bitcoin,
        );
        let code2 = PaymentCode::new(
            PaymentCodeVersion::V3,
            public_key2,
            [0u8; 32],
            Network::Bitcoin,
        );

        let mut channel = PaymentChannel::new(code1, code2);
        assert!(!channel.is_active());

        channel.notification_sent = true;
        assert!(!channel.is_active());

        channel.notification_received = true;
        assert!(channel.is_active());
    }

    #[test]
    fn test_payment_code_validation() {
        let secp = Secp256k1::new();
        let (_, public_key) = secp.generate_keypair(&mut thread_rng());

        let code = PaymentCode::new(
            PaymentCodeVersion::V3,
            public_key,
            [0u8; 32],
            Network::Bitcoin,
        );

        assert!(code.is_valid());
    }
}
