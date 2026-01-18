//! Taproot (BIP 341/342) support
//!
//! This module provides support for Taproot addresses and transactions,
//! enabling enhanced privacy, lower fees, and more flexible smart contracts.

use bitcoin::address::Address;
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::rand::rngs::OsRng;
use bitcoin::secp256k1::{All, PublicKey, SecretKey, XOnlyPublicKey};
use bitcoin::taproot::{TapLeafHash, TapNodeHash, TaprootBuilder, TaprootSpendInfo};
use bitcoin::{Amount, Network, ScriptBuf, Transaction, TxOut};
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
    #[allow(dead_code)]
    secret_key: Option<SecretKey>,
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
    ScriptPath { leaf_index: usize },
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

        // Build the script tree
        let mut builder = TaprootBuilder::new();
        for script in scripts {
            builder = builder
                .add_leaf(0, script)
                .map_err(|e| BitcoinError::Validation(format!("Failed to add leaf: {}", e)))?;
        }

        let spend_info = builder
            .finalize(&self.secp, internal_key)
            .map_err(|_| BitcoinError::Validation("Taproot finalization failed".to_string()))?;

        let merkle_root = spend_info.merkle_root();
        let address = Address::p2tr(&self.secp, internal_key, merkle_root, self.config.network);

        Ok(TaprootAddress {
            address,
            spend_info,
            internal_key,
        })
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

    /// Extract internal key from address
    pub fn extract_internal_key(&self, address: &Address) -> Result<XOnlyPublicKey> {
        if !address.script_pubkey().is_p2tr() {
            return Err(BitcoinError::InvalidAddress(
                "Address is not a Taproot address".to_string(),
            ));
        }

        // Note: This is a simplified version. In production, you'd need to extract
        // the actual internal key from the address structure
        let script_pubkey = address.script_pubkey();
        if script_pubkey.len() != 34 {
            return Err(BitcoinError::InvalidAddress(
                "Invalid Taproot script pubkey length".to_string(),
            ));
        }

        // The x-only pubkey is bytes 2-34 in the script pubkey
        let pubkey_bytes = &script_pubkey.as_bytes()[2..34];
        XOnlyPublicKey::from_slice(pubkey_bytes)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid x-only pubkey: {}", e)))
    }
}

/// Taproot transaction builder
pub struct TaprootTxBuilder {
    #[allow(dead_code)]
    manager: Arc<TaprootManager>,
}

impl TaprootTxBuilder {
    /// Create a new Taproot transaction builder
    pub fn new(manager: Arc<TaprootManager>) -> Self {
        Self { manager }
    }

    /// Create a simple key-path spend
    pub fn create_key_path_spend(
        &self,
        _prev_output: TxOut,
        _destination: Address,
        _amount: Amount,
    ) -> Result<Transaction> {
        // This would create a transaction spending from a Taproot output via key path
        // For now, this is a placeholder for the full implementation
        Err(BitcoinError::Validation(
            "Key path spending not yet implemented".to_string(),
        ))
    }

    /// Create a script-path spend
    pub fn create_script_path_spend(
        &self,
        _prev_output: TxOut,
        _destination: Address,
        _amount: Amount,
        _leaf_index: usize,
    ) -> Result<Transaction> {
        // This would create a transaction spending from a Taproot output via script path
        // For now, this is a placeholder for the full implementation
        Err(BitcoinError::Validation(
            "Script path spending not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taproot_config_defaults() {
        let config = TaprootConfig::default();
        assert_eq!(config.network, Network::Bitcoin);
        assert!(config.enable_script_path);
        assert_eq!(config.max_tree_depth, 128);
    }

    #[test]
    fn test_generate_keypair() {
        let config = TaprootConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = TaprootManager::new(config);

        let keypair = manager.generate_keypair().unwrap();
        assert!(keypair.secret_key.is_some());
    }

    #[test]
    fn test_create_key_path_address() {
        let config = TaprootConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = TaprootManager::new(config);

        let keypair = manager.generate_keypair().unwrap();
        let address = manager
            .create_key_path_address(keypair.internal_key)
            .unwrap();

        assert!(manager.is_taproot_address(&address.address));
    }

    #[test]
    fn test_create_script_path_address() {
        let config = TaprootConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = TaprootManager::new(config);

        let keypair = manager.generate_keypair().unwrap();

        // Create a simple script (OP_TRUE for testing)
        let script = ScriptBuf::from_bytes(vec![0x51]); // OP_TRUE

        let address = manager
            .create_script_path_address(keypair.internal_key, vec![script])
            .unwrap();

        assert!(manager.is_taproot_address(&address.address));
    }

    #[test]
    fn test_validate_taproot_address() {
        let config = TaprootConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = TaprootManager::new(config);

        let keypair = manager.generate_keypair().unwrap();
        let address = manager
            .create_key_path_address(keypair.internal_key)
            .unwrap();

        let is_valid = manager
            .validate_address(&address.address.to_string())
            .unwrap();
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

        let keypair = manager.generate_keypair().unwrap();
        let script = ScriptBuf::from_bytes(vec![0x51]);

        let result = manager.create_script_path_address(keypair.internal_key, vec![script]);
        assert!(result.is_err());
    }
}
