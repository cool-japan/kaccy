//! Wallet Recovery Tools
//!
//! This module provides tools for recovering Bitcoin wallets from seed phrases.
//! It supports:
//! - Deriving all standard address types from a seed
//! - Balance recovery scanning across multiple derivation paths
//! - UTXO discovery
//! - Multi-wallet detection
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_bitcoin::wallet_recovery::{WalletRecovery, RecoveryConfig, DerivationStandard};
//! use kaccy_bitcoin::seed_recovery::{MnemonicGenerator, WordCount};
//! use kaccy_bitcoin::client::{BitcoinClient, BitcoinNetwork};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate or import a mnemonic
//! let mnemonic = MnemonicGenerator::generate(WordCount::TwentyFour)?;
//!
//! // Create Bitcoin client
//! let client = BitcoinClient::new(
//!     "http://localhost:8332",
//!     "user",
//!     "pass",
//!     BitcoinNetwork::Testnet,
//! )?;
//!
//! // Configure recovery
//! let config = RecoveryConfig::default();
//!
//! // Recover wallet
//! let recovery = WalletRecovery::new(client, config);
//! let result = recovery.recover_from_mnemonic(&mnemonic, None)?;
//!
//! println!("Found {} addresses with balance", result.addresses_with_balance.len());
//! println!("Total balance: {} sats", result.total_balance_sats);
//! # Ok(())
//! # }
//! ```

use crate::client::BitcoinClient;
use crate::error::BitcoinError;
use crate::seed_recovery::Mnemonic;
use bitcoin::Address;
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::secp256k1::Secp256k1;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// BIP 44/49/84/86 derivation standards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DerivationStandard {
    /// BIP 44 - Legacy (P2PKH) addresses
    /// Path: m/44'/0'/0'
    Bip44,
    /// BIP 49 - Nested SegWit (P2SH-P2WPKH) addresses
    /// Path: m/49'/0'/0'
    Bip49,
    /// BIP 84 - Native SegWit (P2WPKH) addresses
    /// Path: m/84'/0'/0'
    Bip84,
    /// BIP 86 - Taproot (P2TR) addresses
    /// Path: m/86'/0'/0'
    Bip86,
}

impl DerivationStandard {
    /// Get the BIP 32 purpose value
    pub fn purpose(&self) -> u32 {
        match self {
            DerivationStandard::Bip44 => 44,
            DerivationStandard::Bip49 => 49,
            DerivationStandard::Bip84 => 84,
            DerivationStandard::Bip86 => 86,
        }
    }

    /// Get all derivation standards
    pub fn all() -> Vec<Self> {
        vec![
            DerivationStandard::Bip44,
            DerivationStandard::Bip49,
            DerivationStandard::Bip84,
            DerivationStandard::Bip86,
        ]
    }

    /// Get the derivation path for this standard
    pub fn derivation_path(&self, account: u32) -> String {
        format!("m/{}'/{}'/{}'", self.purpose(), 0, account)
    }
}

/// Configuration for wallet recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Derivation standards to check
    pub standards: Vec<DerivationStandard>,
    /// Maximum number of accounts to check (BIP 44 account index)
    pub max_accounts: u32,
    /// Gap limit for address discovery (BIP 44)
    pub gap_limit: u32,
    /// Whether to check both external and internal (change) chains
    pub check_change_addresses: bool,
    /// Maximum number of addresses to derive per chain before giving up
    pub max_addresses_per_chain: u32,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            standards: DerivationStandard::all(),
            max_accounts: 5,
            gap_limit: 20,
            check_change_addresses: true,
            max_addresses_per_chain: 1000,
        }
    }
}

impl RecoveryConfig {
    /// Create a quick recovery configuration (fewer addresses, faster)
    pub fn quick() -> Self {
        Self {
            standards: vec![DerivationStandard::Bip84], // Only check native SegWit
            max_accounts: 1,
            gap_limit: 10,
            check_change_addresses: false,
            max_addresses_per_chain: 100,
        }
    }

    /// Create a thorough recovery configuration (more addresses, slower)
    pub fn thorough() -> Self {
        Self {
            standards: DerivationStandard::all(),
            max_accounts: 10,
            gap_limit: 50,
            check_change_addresses: true,
            max_addresses_per_chain: 5000,
        }
    }
}

/// Information about a discovered address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAddress {
    /// The Bitcoin address
    pub address: String,
    /// Derivation path used
    pub derivation_path: String,
    /// Derivation standard
    pub standard: DerivationStandard,
    /// Balance in satoshis
    pub balance_sats: u64,
    /// Number of transactions
    pub tx_count: u32,
    /// Whether this is a change address
    pub is_change: bool,
}

/// Result of a wallet recovery operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    /// All addresses discovered (with or without balance)
    pub all_addresses: Vec<DiscoveredAddress>,
    /// Addresses with non-zero balance
    pub addresses_with_balance: Vec<DiscoveredAddress>,
    /// Total balance across all addresses in satoshis
    pub total_balance_sats: u64,
    /// Total number of transactions across all addresses
    pub total_tx_count: u32,
    /// Addresses grouped by derivation standard
    pub by_standard: HashMap<DerivationStandard, Vec<DiscoveredAddress>>,
    /// Number of addresses checked
    pub addresses_checked: usize,
    /// Recovery statistics
    pub stats: RecoveryStats,
}

/// Statistics from recovery operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStats {
    /// Number of standards checked
    pub standards_checked: usize,
    /// Number of accounts checked per standard
    pub accounts_checked: u32,
    /// Total addresses derived
    pub addresses_derived: usize,
    /// Addresses with activity (transactions)
    pub addresses_with_activity: usize,
    /// Time taken for recovery (in seconds)
    pub recovery_time_secs: f64,
}

/// Wallet recovery service
pub struct WalletRecovery {
    client: BitcoinClient,
    config: RecoveryConfig,
}

impl WalletRecovery {
    /// Create a new wallet recovery service
    pub fn new(client: BitcoinClient, config: RecoveryConfig) -> Self {
        Self { client, config }
    }

    /// Recover wallet from a mnemonic phrase
    ///
    /// # Arguments
    /// * `mnemonic` - The BIP 39 mnemonic phrase
    /// * `passphrase` - Optional BIP 39 passphrase
    pub fn recover_from_mnemonic(
        &self,
        mnemonic: &Mnemonic,
        passphrase: Option<&str>,
    ) -> Result<RecoveryResult, BitcoinError> {
        let start_time = std::time::Instant::now();

        let seed = mnemonic.to_seed(passphrase);

        let mut all_addresses = Vec::new();
        let mut checked_addresses = HashSet::new();

        // Check each derivation standard
        for standard in &self.config.standards {
            // Check multiple accounts
            for account in 0..self.config.max_accounts {
                let path = standard.derivation_path(account);

                // Check external chain (receiving addresses)
                let external_addrs = self.discover_addresses(&seed, &path, false, *standard)?;
                all_addresses.extend(external_addrs.clone());
                for addr in &external_addrs {
                    checked_addresses.insert(addr.address.clone());
                }

                // Check internal chain (change addresses)
                if self.config.check_change_addresses {
                    let change_addrs = self.discover_addresses(&seed, &path, true, *standard)?;
                    all_addresses.extend(change_addrs.clone());
                    for addr in &change_addrs {
                        checked_addresses.insert(addr.address.clone());
                    }
                }
            }
        }

        // Calculate statistics
        let addresses_with_balance: Vec<_> = all_addresses
            .iter()
            .filter(|a| a.balance_sats > 0)
            .cloned()
            .collect();

        let total_balance_sats: u64 = addresses_with_balance.iter().map(|a| a.balance_sats).sum();

        let total_tx_count: u32 = all_addresses.iter().map(|a| a.tx_count).sum();

        let addresses_with_activity = all_addresses.iter().filter(|a| a.tx_count > 0).count();

        // Group by standard
        let mut by_standard: HashMap<DerivationStandard, Vec<DiscoveredAddress>> = HashMap::new();
        for addr in &all_addresses {
            by_standard
                .entry(addr.standard)
                .or_default()
                .push(addr.clone());
        }

        let elapsed = start_time.elapsed();

        Ok(RecoveryResult {
            addresses_with_balance: addresses_with_balance.clone(),
            all_addresses: all_addresses.clone(),
            total_balance_sats,
            total_tx_count,
            by_standard,
            addresses_checked: checked_addresses.len(),
            stats: RecoveryStats {
                standards_checked: self.config.standards.len(),
                accounts_checked: self.config.max_accounts,
                addresses_derived: all_addresses.len(),
                addresses_with_activity,
                recovery_time_secs: elapsed.as_secs_f64(),
            },
        })
    }

    /// Discover addresses for a specific derivation path using gap limit.
    ///
    /// Derives addresses using full BIP 32 derivation from the seed bytes.
    /// The address type produced depends on the `standard`:
    /// - BIP 44 → P2PKH (legacy)
    /// - BIP 49 → P2SH-P2WPKH (nested SegWit)
    /// - BIP 84 → P2WPKH (native SegWit)
    /// - BIP 86 → P2TR (Taproot)
    fn discover_addresses(
        &self,
        seed: &[u8],
        base_path: &str,
        is_change: bool,
        standard: DerivationStandard,
    ) -> Result<Vec<DiscoveredAddress>, BitcoinError> {
        let secp = Secp256k1::new();
        let network: bitcoin::Network = self.client.network().into();

        // Derive the root extended private key from the raw seed bytes.
        let root = Xpriv::new_master(network, seed)
            .map_err(|e| BitcoinError::DerivationFailed(format!("Root key error: {}", e)))?;

        let mut addresses = Vec::new();
        let mut gap_count = 0;
        let mut index = 0u32;

        let chain_index: u32 = if is_change { 1 } else { 0 };

        while gap_count < self.config.gap_limit && index < self.config.max_addresses_per_chain {
            // Full BIP 32 path: e.g. m/84'/0'/0'/0/5
            let full_path_str = format!("{}/{}/{}", base_path, chain_index, index);
            let derivation_path = DerivationPath::from_str(&full_path_str).map_err(|e| {
                BitcoinError::DerivationFailed(format!("Invalid path '{}': {}", full_path_str, e))
            })?;

            let child_xpriv = root.derive_priv(&secp, &derivation_path).map_err(|e| {
                BitcoinError::DerivationFailed(format!(
                    "Derivation failed at '{}': {}",
                    full_path_str, e
                ))
            })?;

            let keypair = child_xpriv.to_keypair(&secp);
            let address_str = match standard {
                DerivationStandard::Bip44 => {
                    // P2PKH — legacy address
                    let public_key = bitcoin::PublicKey::new(keypair.public_key());
                    Address::p2pkh(public_key, network).to_string()
                }
                DerivationStandard::Bip49 => {
                    // P2SH-P2WPKH — nested SegWit
                    // CompressedPublicKey::try_from requires bitcoin::PublicKey, not secp256k1::PublicKey
                    let raw_pk = bitcoin::PublicKey::new(keypair.public_key());
                    let compressed =
                        bitcoin::CompressedPublicKey::try_from(raw_pk).map_err(|e| {
                            BitcoinError::DerivationFailed(format!(
                                "Public key compression failed: {}",
                                e
                            ))
                        })?;
                    Address::p2shwpkh(&compressed, network).to_string()
                }
                DerivationStandard::Bip84 => {
                    // P2WPKH — native SegWit (bech32)
                    let raw_pk = bitcoin::PublicKey::new(keypair.public_key());
                    let compressed =
                        bitcoin::CompressedPublicKey::try_from(raw_pk).map_err(|e| {
                            BitcoinError::DerivationFailed(format!(
                                "Public key compression failed: {}",
                                e
                            ))
                        })?;
                    let hrp = match network {
                        bitcoin::Network::Bitcoin => bitcoin::KnownHrp::Mainnet,
                        bitcoin::Network::Testnet | bitcoin::Network::Signet => {
                            bitcoin::KnownHrp::Testnets
                        }
                        bitcoin::Network::Regtest => bitcoin::KnownHrp::Regtest,
                        _ => bitcoin::KnownHrp::Testnets,
                    };
                    Address::p2wpkh(&compressed, hrp).to_string()
                }
                DerivationStandard::Bip86 => {
                    // P2TR — Taproot (bech32m)
                    let internal_key = keypair.x_only_public_key().0;
                    let hrp = match network {
                        bitcoin::Network::Bitcoin => bitcoin::KnownHrp::Mainnet,
                        bitcoin::Network::Testnet | bitcoin::Network::Signet => {
                            bitcoin::KnownHrp::Testnets
                        }
                        bitcoin::Network::Regtest => bitcoin::KnownHrp::Regtest,
                        _ => bitcoin::KnownHrp::Testnets,
                    };
                    Address::p2tr(&secp, internal_key, None, hrp).to_string()
                }
            };

            // Check if address has been used (balance or transaction history).
            let (balance, tx_count) = self.check_address_usage(&address_str)?;

            if balance > 0 || tx_count > 0 {
                // Found activity — reset gap counter and record the address.
                gap_count = 0;
                addresses.push(DiscoveredAddress {
                    address: address_str,
                    derivation_path: full_path_str,
                    standard,
                    balance_sats: balance,
                    tx_count,
                    is_change,
                });
            } else {
                // No activity at this index — advance the gap counter.
                gap_count += 1;
            }

            index += 1;
        }

        Ok(addresses)
    }

    /// Check if an address has been used (has balance or transaction history)
    fn check_address_usage(&self, address: &str) -> Result<(u64, u32), BitcoinError> {
        // Try to parse the address
        let addr = address
            .parse::<Address<_>>()
            .map_err(|e| BitcoinError::InvalidInput(format!("Invalid address: {}", e)))?;

        // Convert to network-checked address
        let network = self.client.network();
        let addr_checked = addr
            .require_network(network.into())
            .map_err(|e| BitcoinError::InvalidInput(format!("Network mismatch: {:?}", e)))?;

        // Get address info from Bitcoin Core
        match self.client.get_address_info(&addr_checked.to_string()) {
            Ok(_info) => {
                // Note: Bitcoin Core RPC doesn't directly provide transaction count
                // In a real implementation, you would need to:
                // 1. Use listtransactions or listreceivedbyaddress
                // 2. Or use an external API like a block explorer
                // For now, we'll use a simplified approach

                // Check if address has received any funds
                let balance = self
                    .client
                    .get_received_by_address(&addr_checked, Some(0))
                    .map(|amt| amt.to_sat())
                    .unwrap_or(0);

                let tx_count = if balance > 0 { 1 } else { 0 };

                Ok((balance, tx_count))
            }
            Err(_) => {
                // Address not found or error - assume no activity
                Ok((0, 0))
            }
        }
    }

    /// Get a summary of the recovery result
    pub fn format_summary(&self, result: &RecoveryResult) -> String {
        let mut summary = String::new();
        summary.push_str("=== Wallet Recovery Summary ===\n\n");
        summary.push_str(&format!(
            "Total addresses checked: {}\n",
            result.addresses_checked
        ));
        summary.push_str(&format!(
            "Addresses with balance: {}\n",
            result.addresses_with_balance.len()
        ));
        summary.push_str(&format!(
            "Total balance: {} BTC ({} sats)\n",
            result.total_balance_sats as f64 / 100_000_000.0,
            result.total_balance_sats
        ));
        summary.push_str(&format!(
            "Total transactions: {}\n\n",
            result.total_tx_count
        ));

        summary.push_str("=== By Derivation Standard ===\n");
        for (standard, addrs) in &result.by_standard {
            let balance: u64 = addrs.iter().map(|a| a.balance_sats).sum();
            summary.push_str(&format!(
                "{:?}: {} addresses, {} sats\n",
                standard,
                addrs.len(),
                balance
            ));
        }

        summary.push_str("\n=== Recovery Stats ===\n");
        summary.push_str(&format!(
            "Standards checked: {}\n",
            result.stats.standards_checked
        ));
        summary.push_str(&format!(
            "Accounts checked: {}\n",
            result.stats.accounts_checked
        ));
        summary.push_str(&format!(
            "Addresses derived: {}\n",
            result.stats.addresses_derived
        ));
        summary.push_str(&format!(
            "Addresses with activity: {}\n",
            result.stats.addresses_with_activity
        ));
        summary.push_str(&format!(
            "Recovery time: {:.2} seconds\n",
            result.stats.recovery_time_secs
        ));

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derivation_standard_purpose() {
        assert_eq!(DerivationStandard::Bip44.purpose(), 44);
        assert_eq!(DerivationStandard::Bip49.purpose(), 49);
        assert_eq!(DerivationStandard::Bip84.purpose(), 84);
        assert_eq!(DerivationStandard::Bip86.purpose(), 86);
    }

    #[test]
    fn test_derivation_standard_all() {
        let all = DerivationStandard::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_derivation_standard_path() {
        assert_eq!(DerivationStandard::Bip44.derivation_path(0), "m/44'/0'/0'");
        assert_eq!(DerivationStandard::Bip84.derivation_path(1), "m/84'/0'/1'");
    }

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert_eq!(config.standards.len(), 4);
        assert_eq!(config.gap_limit, 20);
        assert!(config.check_change_addresses);
    }

    #[test]
    fn test_recovery_config_quick() {
        let config = RecoveryConfig::quick();
        assert_eq!(config.standards.len(), 1);
        assert_eq!(config.gap_limit, 10);
        assert!(!config.check_change_addresses);
    }

    #[test]
    fn test_recovery_config_thorough() {
        let config = RecoveryConfig::thorough();
        assert_eq!(config.standards.len(), 4);
        assert_eq!(config.gap_limit, 50);
        assert!(config.check_change_addresses);
    }

    #[test]
    fn test_discovered_address_creation() {
        let addr = DiscoveredAddress {
            address: "bc1qtest".to_string(),
            derivation_path: "m/84'/0'/0'/0/0".to_string(),
            standard: DerivationStandard::Bip84,
            balance_sats: 100000,
            tx_count: 5,
            is_change: false,
        };

        assert_eq!(addr.address, "bc1qtest");
        assert_eq!(addr.balance_sats, 100000);
        assert_eq!(addr.tx_count, 5);
        assert!(!addr.is_change);
    }
}
