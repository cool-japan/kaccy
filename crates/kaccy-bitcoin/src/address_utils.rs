//! Bitcoin Address Utilities
//!
//! This module provides utilities for analyzing and working with Bitcoin addresses,
//! including address type detection, script analysis, and address metadata extraction.

use crate::error::BitcoinError;
use bitcoin::{Address, Network, ScriptBuf};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Bitcoin address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    /// Pay-to-Public-Key-Hash (Legacy, starts with 1)
    P2PKH,
    /// Pay-to-Script-Hash (Legacy, starts with 3)
    P2SH,
    /// Pay-to-Witness-Public-Key-Hash (Native SegWit, starts with bc1q)
    P2WPKH,
    /// Pay-to-Witness-Script-Hash (Native SegWit, starts with bc1q)
    P2WSH,
    /// Pay-to-Taproot (Taproot, starts with bc1p)
    P2TR,
}

impl AddressType {
    /// Get a human-readable name for the address type
    pub fn name(&self) -> &'static str {
        match self {
            Self::P2PKH => "P2PKH (Legacy)",
            Self::P2SH => "P2SH (Script Hash)",
            Self::P2WPKH => "P2WPKH (Native SegWit)",
            Self::P2WSH => "P2WSH (Native SegWit Script)",
            Self::P2TR => "P2TR (Taproot)",
        }
    }

    /// Check if this is a SegWit address type
    pub fn is_segwit(&self) -> bool {
        matches!(self, Self::P2WPKH | Self::P2WSH | Self::P2TR)
    }

    /// Check if this is a legacy address type
    pub fn is_legacy(&self) -> bool {
        matches!(self, Self::P2PKH | Self::P2SH)
    }

    /// Check if this is a Taproot address
    pub fn is_taproot(&self) -> bool {
        matches!(self, Self::P2TR)
    }

    /// Get the typical witness size for this address type
    pub fn typical_witness_size(&self) -> Option<usize> {
        match self {
            Self::P2PKH => None,       // No witness
            Self::P2SH => None,        // Variable, depends on script
            Self::P2WPKH => Some(107), // 1 + 1 + 72 + 1 + 33
            Self::P2WSH => None,       // Variable, depends on script
            Self::P2TR => Some(65),    // 1 + 1 + 64 (Schnorr signature)
        }
    }
}

/// Address metadata and analysis information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    /// The original address string
    pub address: String,

    /// Address type
    pub address_type: AddressType,

    /// Network (mainnet, testnet, etc.)
    pub network: Network,

    /// Script public key
    pub script_pubkey: ScriptBuf,

    /// Whether this is a multisig address (for P2SH/P2WSH)
    pub is_multisig: bool,

    /// Estimated input size in vbytes when spending from this address
    pub estimated_input_vsize: usize,

    /// Whether this address supports RBF by default
    pub supports_rbf: bool,
}

impl AddressInfo {
    /// Analyze a Bitcoin address and extract information
    pub fn analyze(address: &str) -> Result<Self, BitcoinError> {
        let unchecked_addr = Address::from_str(address)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid address: {}", e)))?;

        // Detect network from address string
        let network = Self::detect_network(address)?;

        // Check the address against the detected network
        let addr = unchecked_addr
            .require_network(network)
            .map_err(|_| BitcoinError::InvalidAddress("Address network mismatch".to_string()))?;

        let script_pubkey = addr.script_pubkey();
        let address_type = Self::detect_type(&addr)?;

        // Estimate input size based on address type
        let estimated_input_vsize = match address_type {
            AddressType::P2PKH => 148, // ~148 vbytes
            AddressType::P2SH => 91,   // ~91 vbytes for P2SH-P2WPKH
            AddressType::P2WPKH => 68, // ~68 vbytes
            AddressType::P2WSH => 104, // ~104 vbytes (variable)
            AddressType::P2TR => 58,   // ~58 vbytes (key path spend)
        };

        // P2SH and P2WSH can be multisig, but we can't tell without the script
        let is_multisig = matches!(address_type, AddressType::P2SH | AddressType::P2WSH);

        Ok(Self {
            address: address.to_string(),
            address_type,
            network,
            script_pubkey,
            is_multisig,
            estimated_input_vsize,
            supports_rbf: true, // All address types support RBF
        })
    }

    /// Detect network from address string
    fn detect_network(address: &str) -> Result<Network, BitcoinError> {
        if address.starts_with("bc1") || address.starts_with('1') || address.starts_with('3') {
            Ok(Network::Bitcoin)
        } else if address.starts_with("tb1")
            || address.starts_with('m')
            || address.starts_with('n')
            || address.starts_with('2')
        {
            Ok(Network::Testnet)
        } else if address.starts_with("bcrt1") {
            Ok(Network::Regtest)
        } else {
            Err(BitcoinError::InvalidAddress(
                "Unable to detect network from address".to_string(),
            ))
        }
    }

    /// Detect the address type
    fn detect_type(addr: &Address) -> Result<AddressType, BitcoinError> {
        let script = addr.script_pubkey();

        if script.is_p2pkh() {
            Ok(AddressType::P2PKH)
        } else if script.is_p2sh() {
            Ok(AddressType::P2SH)
        } else if script.is_p2wpkh() {
            Ok(AddressType::P2WPKH)
        } else if script.is_p2wsh() {
            Ok(AddressType::P2WSH)
        } else if script.is_p2tr() {
            Ok(AddressType::P2TR)
        } else {
            Err(BitcoinError::InvalidAddress(
                "Unknown address type".to_string(),
            ))
        }
    }

    /// Check if this address is on the given network
    pub fn is_network(&self, network: Network) -> bool {
        self.network == network
    }

    /// Get the fee cost (in sats) for spending from this address at a given fee rate
    ///
    /// # Arguments
    /// * `fee_rate` - Fee rate in sat/vbyte
    pub fn spending_fee_cost(&self, fee_rate: f64) -> u64 {
        (self.estimated_input_vsize as f64 * fee_rate).ceil() as u64
    }

    /// Check if this address is more private than another
    ///
    /// SegWit and Taproot addresses are generally more private
    pub fn is_more_private_than(&self, other: &AddressInfo) -> bool {
        match (self.address_type, other.address_type) {
            // Taproot is most private
            (AddressType::P2TR, AddressType::P2TR) => false,
            (AddressType::P2TR, _) => true,
            (_, AddressType::P2TR) => false,

            // SegWit is more private than legacy
            _ if self.address_type.is_segwit() && !other.address_type.is_segwit() => true,
            _ if !self.address_type.is_segwit() && other.address_type.is_segwit() => false,

            // Same privacy level
            _ => false,
        }
    }

    /// Get a privacy score (0-100, higher is better)
    pub fn privacy_score(&self) -> u8 {
        match self.address_type {
            AddressType::P2TR => 100, // Best privacy (looks like any other Taproot spend)
            AddressType::P2WPKH => 80, // Good privacy (SegWit)
            AddressType::P2WSH => 75, // Good privacy but reveals script hash
            AddressType::P2SH => 60,  // Moderate (could be multisig or SegWit wrapper)
            AddressType::P2PKH => 40, // Poor (legacy, larger on-chain footprint)
        }
    }
}

/// Address comparison utilities
pub struct AddressComparator;

impl AddressComparator {
    /// Compare two addresses for cost efficiency
    ///
    /// Returns the more cost-efficient address (cheaper to spend from)
    pub fn more_efficient<'a>(
        addr1: &'a AddressInfo,
        addr2: &'a AddressInfo,
        fee_rate: f64,
    ) -> &'a AddressInfo {
        let cost1 = addr1.spending_fee_cost(fee_rate);
        let cost2 = addr2.spending_fee_cost(fee_rate);

        if cost1 <= cost2 { addr1 } else { addr2 }
    }

    /// Compare two addresses for privacy
    ///
    /// Returns the more private address
    pub fn more_private<'a>(addr1: &'a AddressInfo, addr2: &'a AddressInfo) -> &'a AddressInfo {
        if addr1.is_more_private_than(addr2) {
            addr1
        } else {
            addr2
        }
    }

    /// Get the recommended address type for new wallets
    pub fn recommended_type() -> AddressType {
        AddressType::P2TR // Taproot is the most modern and private
    }
}

/// Batch address analyzer
pub struct AddressBatchAnalyzer {
    addresses: Vec<AddressInfo>,
}

impl AddressBatchAnalyzer {
    /// Create a new batch analyzer
    pub fn new() -> Self {
        Self {
            addresses: Vec::new(),
        }
    }

    /// Add an address to analyze
    pub fn add(&mut self, address: &str) -> Result<(), BitcoinError> {
        let info = AddressInfo::analyze(address)?;
        self.addresses.push(info);
        Ok(())
    }

    /// Get statistics about the addresses
    pub fn statistics(&self) -> AddressBatchStatistics {
        let mut stats = AddressBatchStatistics::default();

        for addr in &self.addresses {
            match addr.address_type {
                AddressType::P2PKH => stats.p2pkh_count += 1,
                AddressType::P2SH => stats.p2sh_count += 1,
                AddressType::P2WPKH => stats.p2wpkh_count += 1,
                AddressType::P2WSH => stats.p2wsh_count += 1,
                AddressType::P2TR => stats.p2tr_count += 1,
            }

            if addr.address_type.is_segwit() {
                stats.segwit_count += 1;
            }

            if addr.address_type.is_legacy() {
                stats.legacy_count += 1;
            }

            stats.total_count += 1;
            stats.average_privacy_score += addr.privacy_score() as u32;
        }

        if stats.total_count > 0 {
            stats.average_privacy_score /= stats.total_count as u32;
        }

        stats
    }

    /// Find addresses that should be upgraded (legacy to SegWit/Taproot)
    pub fn find_upgradeable(&self) -> Vec<&AddressInfo> {
        self.addresses
            .iter()
            .filter(|addr| addr.address_type.is_legacy())
            .collect()
    }

    /// Get the most private address type in the batch
    pub fn most_private_type(&self) -> Option<AddressType> {
        self.addresses
            .iter()
            .max_by_key(|addr| addr.privacy_score())
            .map(|addr| addr.address_type)
    }
}

impl Default for AddressBatchAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics from batch address analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressBatchStatistics {
    pub total_count: usize,
    pub p2pkh_count: usize,
    pub p2sh_count: usize,
    pub p2wpkh_count: usize,
    pub p2wsh_count: usize,
    pub p2tr_count: usize,
    pub segwit_count: usize,
    pub legacy_count: usize,
    pub average_privacy_score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_type_properties() {
        assert_eq!(AddressType::P2PKH.name(), "P2PKH (Legacy)");
        assert!(!AddressType::P2PKH.is_segwit());
        assert!(AddressType::P2PKH.is_legacy());

        assert!(AddressType::P2WPKH.is_segwit());
        assert!(!AddressType::P2WPKH.is_legacy());

        assert!(AddressType::P2TR.is_taproot());
        assert!(AddressType::P2TR.is_segwit());
    }

    #[test]
    fn test_address_analysis_p2wpkh() {
        let info = AddressInfo::analyze("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh").unwrap();
        assert_eq!(info.address_type, AddressType::P2WPKH);
        assert_eq!(info.network, Network::Bitcoin);
        assert!(info.supports_rbf);
        assert_eq!(info.estimated_input_vsize, 68);
    }

    #[test]
    fn test_spending_fee_cost() {
        let info = AddressInfo::analyze("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh").unwrap();
        let fee = info.spending_fee_cost(10.0); // 10 sat/vbyte
        assert_eq!(fee, 680); // 68 vbytes * 10 sat/vbyte
    }

    #[test]
    fn test_privacy_score() {
        let p2pkh = AddressInfo::analyze("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap();
        let p2wpkh = AddressInfo::analyze("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh").unwrap();

        assert!(p2wpkh.privacy_score() > p2pkh.privacy_score());
    }

    #[test]
    fn test_batch_analyzer() {
        let mut analyzer = AddressBatchAnalyzer::new();
        analyzer
            .add("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .unwrap();
        analyzer.add("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap();

        let stats = analyzer.statistics();
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.p2wpkh_count, 1);
        assert_eq!(stats.p2pkh_count, 1);
        assert_eq!(stats.segwit_count, 1);
        assert_eq!(stats.legacy_count, 1);
    }

    #[test]
    fn test_find_upgradeable() {
        let mut analyzer = AddressBatchAnalyzer::new();
        analyzer
            .add("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .unwrap();
        analyzer.add("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap();

        let upgradeable = analyzer.find_upgradeable();
        assert_eq!(upgradeable.len(), 1);
        assert_eq!(upgradeable[0].address_type, AddressType::P2PKH);
    }

    #[test]
    fn test_address_comparator() {
        let recommended = AddressComparator::recommended_type();
        assert_eq!(recommended, AddressType::P2TR);
    }

    #[test]
    fn test_witness_size() {
        assert_eq!(AddressType::P2WPKH.typical_witness_size(), Some(107));
        assert_eq!(AddressType::P2TR.typical_witness_size(), Some(65));
        assert_eq!(AddressType::P2PKH.typical_witness_size(), None);
    }
}
