//! BIP84 HD Wallet Support
//!
//! Implements native SegWit (bech32) address derivation using BIP84.
//! Derivation path: m/84'/0'/0'/0/{index} for receiving addresses
//!
//! This module provides:
//! - XPUB/ZPUB parsing and validation
//! - Deterministic address generation
//! - Address caching for performance
//! - Address validation

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::RwLock;

use bitcoin::bip32::{DerivationPath, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, CompressedPublicKey, Network, PublicKey};

use crate::error::{BitcoinError, Result};

/// BIP84 derivation path constants
pub mod derivation {
    /// BIP84 purpose (native SegWit)
    pub const PURPOSE: u32 = 84;
    /// Bitcoin mainnet coin type
    pub const COIN_TYPE_MAINNET: u32 = 0;
    /// Bitcoin testnet coin type
    pub const COIN_TYPE_TESTNET: u32 = 1;
    /// Default account index
    pub const ACCOUNT: u32 = 0;
    /// External chain (receiving addresses)
    pub const EXTERNAL_CHAIN: u32 = 0;
    /// Internal chain (change addresses)
    pub const INTERNAL_CHAIN: u32 = 1;
}

/// HD Wallet configuration
#[derive(Debug, Clone)]
pub struct HdWalletConfig {
    /// The extended public key (xpub/zpub)
    pub xpub: String,
    /// Bitcoin network
    pub network: Network,
    /// Gap limit for address discovery (default: 20)
    pub gap_limit: u32,
    /// Whether to cache derived addresses
    pub enable_cache: bool,
}

impl Default for HdWalletConfig {
    fn default() -> Self {
        Self {
            xpub: String::new(),
            network: Network::Bitcoin,
            gap_limit: 20,
            enable_cache: true,
        }
    }
}

impl HdWalletConfig {
    /// Create config for mainnet
    pub fn mainnet(xpub: String) -> Self {
        Self {
            xpub,
            network: Network::Bitcoin,
            ..Default::default()
        }
    }

    /// Create config for testnet
    pub fn testnet(xpub: String) -> Self {
        Self {
            xpub,
            network: Network::Testnet,
            ..Default::default()
        }
    }

    /// Set gap limit
    pub fn with_gap_limit(mut self, gap_limit: u32) -> Self {
        self.gap_limit = gap_limit;
        self
    }
}

/// Address cache entry
#[derive(Debug, Clone)]
pub struct CachedAddress {
    pub address: String,
    pub index: u32,
    pub is_change: bool,
    pub used: bool,
}

/// BIP84 HD Wallet for deterministic address generation
///
/// Implements BIP84 (native SegWit) address derivation from an extended public key.
///
/// # Examples
///
/// ```no_run
/// use kaccy_bitcoin::{HdWallet, HdWalletConfig};
/// use bitcoin::Network;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = HdWalletConfig {
///     xpub: "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string(),
///     network: Network::Testnet,
///     gap_limit: 20,
///     enable_cache: true,
/// };
///
/// let wallet = HdWallet::new(config)?;
///
/// // Derive a receiving address (external chain, index 0)
/// let address = wallet.derive_address(0, 0)?;
/// println!("Address: {}", address);
/// # Ok(())
/// # }
/// ```
pub struct HdWallet {
    xpub: Xpub,
    network: Network,
    secp: Secp256k1<bitcoin::secp256k1::All>,
    /// Cache of derived addresses: (chain, index) -> address
    address_cache: RwLock<HashMap<(u32, u32), CachedAddress>>,
    /// Next index to use for external chain
    next_external_index: RwLock<u32>,
    /// Next index to use for internal chain
    next_internal_index: RwLock<u32>,
    config: HdWalletConfig,
}

impl HdWallet {
    /// Create a new HD wallet from configuration
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kaccy_bitcoin::{HdWallet, HdWalletConfig};
    /// use bitcoin::Network;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = HdWalletConfig::testnet(
    ///     "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string()
    /// );
    /// let wallet = HdWallet::new(config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config: HdWalletConfig) -> Result<Self> {
        let xpub = Self::parse_xpub(&config.xpub)?;

        Ok(Self {
            xpub,
            network: config.network,
            secp: Secp256k1::new(),
            address_cache: RwLock::new(HashMap::new()),
            next_external_index: RwLock::new(0),
            next_internal_index: RwLock::new(0),
            config,
        })
    }

    /// Parse an xpub/zpub string into an Xpub
    fn parse_xpub(xpub_str: &str) -> Result<Xpub> {
        // Handle both xpub and zpub formats
        // zpub is the BIP84 specific format, but internally we convert to xpub
        let xpub_str = if xpub_str.starts_with("zpub") {
            // Convert zpub to xpub format (they're the same key, different encoding)
            // For simplicity, we'll just try to parse directly
            // In production, you'd want proper conversion
            xpub_str
        } else {
            xpub_str
        };

        Xpub::from_str(xpub_str)
            .map_err(|e| BitcoinError::InvalidXpub(format!("Failed to parse xpub: {}", e)))
    }

    /// Validate an xpub string
    pub fn validate_xpub(xpub_str: &str) -> bool {
        Self::parse_xpub(xpub_str).is_ok()
    }

    /// Derive a child public key at the given path
    fn derive_child(&self, chain: u32, index: u32) -> Result<PublicKey> {
        // Build derivation path: chain/index (relative to the xpub)
        let path_str = format!("{}/{}", chain, index);
        let path = DerivationPath::from_str(&format!("m/{}", path_str))
            .map_err(|e| BitcoinError::DerivationFailed(format!("Invalid path: {}", e)))?;

        // Derive the child key
        let child_xpub = self
            .xpub
            .derive_pub(&self.secp, &path)
            .map_err(|e| BitcoinError::DerivationFailed(format!("Derivation failed: {}", e)))?;

        Ok(PublicKey::new(child_xpub.public_key))
    }

    /// Create a native SegWit (bech32) address from a public key
    fn create_address(&self, pubkey: &PublicKey) -> Result<Address> {
        let compressed = CompressedPublicKey(pubkey.inner);
        let address = Address::p2wpkh(&compressed, self.network);
        Ok(address)
    }

    /// Derive an address at the given chain and index
    pub fn derive_address(&self, chain: u32, index: u32) -> Result<String> {
        // Check cache first
        if self.config.enable_cache {
            let cache = self.address_cache.read().unwrap();
            if let Some(cached) = cache.get(&(chain, index)) {
                return Ok(cached.address.clone());
            }
        }

        // Derive the address
        let pubkey = self.derive_child(chain, index)?;
        let address = self.create_address(&pubkey)?;
        let address_str = address.to_string();

        // Cache the result
        if self.config.enable_cache {
            let mut cache = self.address_cache.write().unwrap();
            cache.insert(
                (chain, index),
                CachedAddress {
                    address: address_str.clone(),
                    index,
                    is_change: chain == derivation::INTERNAL_CHAIN,
                    used: false,
                },
            );
        }

        Ok(address_str)
    }

    /// Get the next unused external (receiving) address
    pub fn get_next_receiving_address(&self) -> Result<AddressInfo> {
        let index = {
            let mut next = self.next_external_index.write().unwrap();
            let current = *next;
            *next += 1;
            current
        };

        let address = self.derive_address(derivation::EXTERNAL_CHAIN, index)?;

        Ok(AddressInfo {
            address,
            index,
            chain: derivation::EXTERNAL_CHAIN,
            derivation_path: format!(
                "m/{}'/{}'/{}'/{}/{}",
                derivation::PURPOSE,
                if self.network == Network::Bitcoin {
                    derivation::COIN_TYPE_MAINNET
                } else {
                    derivation::COIN_TYPE_TESTNET
                },
                derivation::ACCOUNT,
                derivation::EXTERNAL_CHAIN,
                index
            ),
        })
    }

    /// Get the next unused internal (change) address
    pub fn get_next_change_address(&self) -> Result<AddressInfo> {
        let index = {
            let mut next = self.next_internal_index.write().unwrap();
            let current = *next;
            *next += 1;
            current
        };

        let address = self.derive_address(derivation::INTERNAL_CHAIN, index)?;

        Ok(AddressInfo {
            address,
            index,
            chain: derivation::INTERNAL_CHAIN,
            derivation_path: format!(
                "m/{}'/{}'/{}'/{}/{}",
                derivation::PURPOSE,
                if self.network == Network::Bitcoin {
                    derivation::COIN_TYPE_MAINNET
                } else {
                    derivation::COIN_TYPE_TESTNET
                },
                derivation::ACCOUNT,
                derivation::INTERNAL_CHAIN,
                index
            ),
        })
    }

    /// Derive a specific address by order index
    /// Uses a deterministic mapping from order ID to address index
    pub fn derive_order_address(&self, order_index: u32) -> Result<AddressInfo> {
        let address = self.derive_address(derivation::EXTERNAL_CHAIN, order_index)?;

        Ok(AddressInfo {
            address,
            index: order_index,
            chain: derivation::EXTERNAL_CHAIN,
            derivation_path: format!(
                "m/{}'/{}'/{}'/{}/{}",
                derivation::PURPOSE,
                if self.network == Network::Bitcoin {
                    derivation::COIN_TYPE_MAINNET
                } else {
                    derivation::COIN_TYPE_TESTNET
                },
                derivation::ACCOUNT,
                derivation::EXTERNAL_CHAIN,
                order_index
            ),
        })
    }

    /// Mark an address as used
    pub fn mark_address_used(&self, chain: u32, index: u32) {
        if self.config.enable_cache {
            let mut cache = self.address_cache.write().unwrap();
            if let Some(entry) = cache.get_mut(&(chain, index)) {
                entry.used = true;
            }
        }
    }

    /// Get all cached addresses
    pub fn get_cached_addresses(&self) -> Vec<CachedAddress> {
        let cache = self.address_cache.read().unwrap();
        cache.values().cloned().collect()
    }

    /// Get cached addresses for a specific chain
    pub fn get_chain_addresses(&self, chain: u32) -> Vec<CachedAddress> {
        let cache = self.address_cache.read().unwrap();
        cache
            .iter()
            .filter(|((c, _), _)| *c == chain)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Validate that an address belongs to this wallet
    pub fn is_wallet_address(&self, address: &str, max_index: u32) -> Result<Option<AddressInfo>> {
        // Check external chain
        for index in 0..max_index {
            let derived = self.derive_address(derivation::EXTERNAL_CHAIN, index)?;
            if derived == address {
                return Ok(Some(AddressInfo {
                    address: derived,
                    index,
                    chain: derivation::EXTERNAL_CHAIN,
                    derivation_path: format!(
                        "m/{}'/{}'/{}'/{}/{}",
                        derivation::PURPOSE,
                        if self.network == Network::Bitcoin {
                            derivation::COIN_TYPE_MAINNET
                        } else {
                            derivation::COIN_TYPE_TESTNET
                        },
                        derivation::ACCOUNT,
                        derivation::EXTERNAL_CHAIN,
                        index
                    ),
                }));
            }
        }

        // Check internal chain
        for index in 0..max_index {
            let derived = self.derive_address(derivation::INTERNAL_CHAIN, index)?;
            if derived == address {
                return Ok(Some(AddressInfo {
                    address: derived,
                    index,
                    chain: derivation::INTERNAL_CHAIN,
                    derivation_path: format!(
                        "m/{}'/{}'/{}'/{}/{}",
                        derivation::PURPOSE,
                        if self.network == Network::Bitcoin {
                            derivation::COIN_TYPE_MAINNET
                        } else {
                            derivation::COIN_TYPE_TESTNET
                        },
                        derivation::ACCOUNT,
                        derivation::INTERNAL_CHAIN,
                        index
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Get the current index for external chain
    pub fn current_external_index(&self) -> u32 {
        *self.next_external_index.read().unwrap()
    }

    /// Get the current index for internal chain
    pub fn current_internal_index(&self) -> u32 {
        *self.next_internal_index.read().unwrap()
    }

    /// Set the starting index for external chain
    pub fn set_external_index(&self, index: u32) {
        let mut next = self.next_external_index.write().unwrap();
        *next = index;
    }

    /// Set the starting index for internal chain
    pub fn set_internal_index(&self, index: u32) {
        let mut next = self.next_internal_index.write().unwrap();
        *next = index;
    }

    /// Pre-derive addresses up to a certain index for caching
    pub fn preload_addresses(&self, count: u32) -> Result<()> {
        for i in 0..count {
            self.derive_address(derivation::EXTERNAL_CHAIN, i)?;
        }
        Ok(())
    }
}

/// Information about a derived address
#[derive(Debug, Clone, serde::Serialize)]
pub struct AddressInfo {
    /// The derived address
    pub address: String,
    /// Index in the derivation path
    pub index: u32,
    /// Chain (0 = external, 1 = internal)
    pub chain: u32,
    /// Full derivation path
    pub derivation_path: String,
}

/// Address book for mapping addresses to metadata
pub struct AddressBook {
    /// Map of address -> label
    labels: RwLock<HashMap<String, String>>,
    /// Map of order_id -> address
    order_addresses: RwLock<HashMap<String, String>>,
}

impl Default for AddressBook {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressBook {
    pub fn new() -> Self {
        Self {
            labels: RwLock::new(HashMap::new()),
            order_addresses: RwLock::new(HashMap::new()),
        }
    }

    /// Add a label to an address
    pub fn set_label(&self, address: &str, label: &str) {
        let mut labels = self.labels.write().unwrap();
        labels.insert(address.to_string(), label.to_string());
    }

    /// Get the label for an address
    pub fn get_label(&self, address: &str) -> Option<String> {
        let labels = self.labels.read().unwrap();
        labels.get(address).cloned()
    }

    /// Associate an order with an address
    pub fn set_order_address(&self, order_id: &str, address: &str) {
        let mut order_addresses = self.order_addresses.write().unwrap();
        order_addresses.insert(order_id.to_string(), address.to_string());
    }

    /// Get the address for an order
    pub fn get_order_address(&self, order_id: &str) -> Option<String> {
        let order_addresses = self.order_addresses.read().unwrap();
        order_addresses.get(order_id).cloned()
    }

    /// Get the order ID for an address
    pub fn get_address_order(&self, address: &str) -> Option<String> {
        let order_addresses = self.order_addresses.read().unwrap();
        for (order_id, addr) in order_addresses.iter() {
            if addr == address {
                return Some(order_id.clone());
            }
        }
        None
    }
}
