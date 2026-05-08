//! Output Descriptor support for modern Bitcoin wallet management
//!
//! This module implements Bitcoin Output Descriptors, which provide a standardized
//! way to describe how scripts (and their associated addresses) should be generated.
//! Descriptors are more flexible and explicit than traditional wallet formats.

use bitcoin::{Address, Network};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// Descriptor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DescriptorType {
    /// Pay-to-Public-Key-Hash (legacy)
    Pkh,
    /// Pay-to-Witness-Public-Key-Hash (native SegWit)
    Wpkh,
    /// Pay-to-Script-Hash wrapping WPKH (nested SegWit)
    ShWpkh,
    /// Pay-to-Taproot (BIP 341)
    Tr,
    /// Multi-signature
    Multi,
    /// Sorted multi-signature
    SortedMulti,
}

impl DescriptorType {
    /// Get the descriptor prefix string
    pub fn prefix(&self) -> &'static str {
        match self {
            DescriptorType::Pkh => "pkh",
            DescriptorType::Wpkh => "wpkh",
            DescriptorType::ShWpkh => "sh(wpkh",
            DescriptorType::Tr => "tr",
            DescriptorType::Multi => "multi",
            DescriptorType::SortedMulti => "sortedmulti",
        }
    }

    /// Check if this descriptor type supports Taproot
    pub fn is_taproot(&self) -> bool {
        matches!(self, DescriptorType::Tr)
    }

    /// Check if this descriptor type supports SegWit
    pub fn is_segwit(&self) -> bool {
        matches!(
            self,
            DescriptorType::Wpkh | DescriptorType::ShWpkh | DescriptorType::Tr
        )
    }
}

/// Output descriptor configuration
#[derive(Debug, Clone)]
pub struct DescriptorConfig {
    /// Descriptor type
    pub descriptor_type: DescriptorType,
    /// Network
    pub network: Network,
    /// Enable checksum validation
    pub validate_checksum: bool,
}

impl Default for DescriptorConfig {
    fn default() -> Self {
        Self {
            descriptor_type: DescriptorType::Wpkh,
            network: Network::Bitcoin,
            validate_checksum: true,
        }
    }
}

/// Output descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDescriptor {
    /// Descriptor string (without checksum)
    pub descriptor: String,
    /// Descriptor type
    pub descriptor_type: DescriptorType,
    /// Checksum (if present)
    pub checksum: Option<String>,
    /// Network
    pub network: Network,
}

impl OutputDescriptor {
    /// Create a new output descriptor
    pub fn new(descriptor: String, descriptor_type: DescriptorType, network: Network) -> Self {
        // Try to parse checksum from descriptor
        let (desc, checksum) = if let Some(idx) = descriptor.rfind('#') {
            let (d, c) = descriptor.split_at(idx);
            (d.to_string(), Some(c[1..].to_string()))
        } else {
            (descriptor, None)
        };

        Self {
            descriptor: desc,
            descriptor_type,
            checksum,
            network,
        }
    }

    /// Get the full descriptor string with checksum
    pub fn to_string_with_checksum(&self) -> String {
        match &self.checksum {
            Some(cs) => format!("{}#{}", self.descriptor, cs),
            None => self.descriptor.clone(),
        }
    }

    /// Validate the descriptor checksum
    pub fn validate_checksum(&self) -> Result<bool> {
        // In a full implementation, this would compute and verify the descriptor checksum
        // For now, we just check if it exists
        Ok(self.checksum.is_some())
    }

    /// Derive an address at a specific index
    pub fn derive_address(&self, _index: u32) -> Result<Address> {
        // This is a simplified implementation
        // In production, you'd use the miniscript crate or Bitcoin Core RPC

        // For now, we return an error indicating this needs Bitcoin Core
        Err(BitcoinError::Validation(
            "Address derivation requires Bitcoin Core RPC integration".to_string(),
        ))
    }
}

/// Descriptor wallet manager
pub struct DescriptorWallet {
    #[allow(dead_code)]
    config: DescriptorConfig,
    #[allow(dead_code)]
    client: Arc<BitcoinClient>,
    /// Map of descriptor names to descriptors
    descriptors: Arc<RwLock<HashMap<String, OutputDescriptor>>>,
}

impl DescriptorWallet {
    /// Create a new descriptor wallet
    pub fn new(config: DescriptorConfig, client: Arc<BitcoinClient>) -> Self {
        Self {
            config,
            client,
            descriptors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Import a descriptor
    pub fn import_descriptor(
        &self,
        name: String,
        descriptor: String,
        descriptor_type: DescriptorType,
    ) -> Result<()> {
        let output_desc = OutputDescriptor::new(descriptor, descriptor_type, self.config.network);

        // Validate checksum if required
        if self.config.validate_checksum {
            output_desc.validate_checksum()?;
        }

        let mut descriptors = self.descriptors.write().unwrap_or_else(|e| e.into_inner());
        descriptors.insert(name, output_desc);

        Ok(())
    }

    /// Get a descriptor by name
    pub fn get_descriptor(&self, name: &str) -> Option<OutputDescriptor> {
        let descriptors = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        descriptors.get(name).cloned()
    }

    /// List all descriptors
    pub fn list_descriptors(&self) -> Vec<(String, OutputDescriptor)> {
        let descriptors = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        descriptors
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Remove a descriptor
    pub fn remove_descriptor(&self, name: &str) -> Result<()> {
        let mut descriptors = self.descriptors.write().unwrap_or_else(|e| e.into_inner());
        descriptors
            .remove(name)
            .ok_or_else(|| BitcoinError::Validation(format!("Descriptor {} not found", name)))?;
        Ok(())
    }

    /// Create a single-key WPKH descriptor
    pub fn create_wpkh_descriptor(pubkey: &str, network: Network) -> Result<OutputDescriptor> {
        let descriptor = format!("wpkh({})", pubkey);
        Ok(OutputDescriptor::new(
            descriptor,
            DescriptorType::Wpkh,
            network,
        ))
    }

    /// Create a Taproot descriptor
    pub fn create_tr_descriptor(pubkey: &str, network: Network) -> Result<OutputDescriptor> {
        let descriptor = format!("tr({})", pubkey);
        Ok(OutputDescriptor::new(
            descriptor,
            DescriptorType::Tr,
            network,
        ))
    }

    /// Create a multisig descriptor
    pub fn create_multisig_descriptor(
        threshold: usize,
        pubkeys: &[String],
        network: Network,
    ) -> Result<OutputDescriptor> {
        if threshold == 0 || threshold > pubkeys.len() {
            return Err(BitcoinError::Validation(
                "Invalid multisig threshold".to_string(),
            ));
        }

        let keys_str = pubkeys.join(",");
        let descriptor = format!("wsh(multi({},{}))", threshold, keys_str);

        Ok(OutputDescriptor::new(
            descriptor,
            DescriptorType::Multi,
            network,
        ))
    }

    /// Get the network
    pub fn network(&self) -> Network {
        self.config.network
    }
}

/// Descriptor range for derivation
#[derive(Debug, Clone, Copy)]
pub struct DescriptorRange {
    /// Start index (inclusive)
    pub start: u32,
    /// End index (inclusive)
    pub end: u32,
}

impl DescriptorRange {
    /// Create a new range
    pub fn new(start: u32, end: u32) -> Result<Self> {
        if start > end {
            return Err(BitcoinError::Validation(
                "Invalid range: start > end".to_string(),
            ));
        }
        Ok(Self { start, end })
    }

    /// Get the number of addresses in this range
    pub fn count(&self) -> u32 {
        self.end - self.start + 1
    }

    /// Create a range for a single index
    pub fn single(index: u32) -> Self {
        Self {
            start: index,
            end: index,
        }
    }

    /// Create a range from 0 to count-1
    pub fn from_count(count: u32) -> Self {
        Self {
            start: 0,
            end: count.saturating_sub(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptor_type_prefix() {
        assert_eq!(DescriptorType::Wpkh.prefix(), "wpkh");
        assert_eq!(DescriptorType::Pkh.prefix(), "pkh");
        assert_eq!(DescriptorType::Tr.prefix(), "tr");
    }

    #[test]
    fn test_descriptor_type_is_taproot() {
        assert!(DescriptorType::Tr.is_taproot());
        assert!(!DescriptorType::Wpkh.is_taproot());
        assert!(!DescriptorType::Pkh.is_taproot());
    }

    #[test]
    fn test_descriptor_type_is_segwit() {
        assert!(DescriptorType::Wpkh.is_segwit());
        assert!(DescriptorType::ShWpkh.is_segwit());
        assert!(DescriptorType::Tr.is_segwit());
        assert!(!DescriptorType::Pkh.is_segwit());
    }

    #[test]
    fn test_output_descriptor_creation() {
        let desc = OutputDescriptor::new(
            "wpkh([d34db33f/84'/0'/0']xpub...)".to_string(),
            DescriptorType::Wpkh,
            Network::Bitcoin,
        );
        assert_eq!(desc.descriptor_type, DescriptorType::Wpkh);
        assert_eq!(desc.network, Network::Bitcoin);
    }

    #[test]
    fn test_output_descriptor_with_checksum() {
        let desc = OutputDescriptor::new(
            "wpkh(xpub...)#12345678".to_string(),
            DescriptorType::Wpkh,
            Network::Bitcoin,
        );
        assert_eq!(desc.checksum, Some("12345678".to_string()));
        assert_eq!(desc.to_string_with_checksum(), "wpkh(xpub...)#12345678");
    }

    #[test]
    fn test_create_wpkh_descriptor() {
        let desc = DescriptorWallet::create_wpkh_descriptor(
            "xpub6D4BDPcP2GT577Vvch3R8wDkScZWzQzMMUm3PWbmWvVJrZwQY4VUNgqFJPMM3No2dFDFGTsxxpG5uJh7n7epu4trkrX7x7DogT5Uv6fcLW5",
            Network::Bitcoin,
        )
        .unwrap();

        assert_eq!(desc.descriptor_type, DescriptorType::Wpkh);
        assert!(desc.descriptor.starts_with("wpkh("));
    }

    #[test]
    fn test_create_tr_descriptor() {
        let desc = DescriptorWallet::create_tr_descriptor(
            "xpub6D4BDPcP2GT577Vvch3R8wDkScZWzQzMMUm3PWbmWvVJrZwQY4VUNgqFJPMM3No2dFDFGTsxxpG5uJh7n7epu4trkrX7x7DogT5Uv6fcLW5",
            Network::Bitcoin,
        )
        .unwrap();

        assert_eq!(desc.descriptor_type, DescriptorType::Tr);
        assert!(desc.descriptor.starts_with("tr("));
    }

    #[test]
    fn test_create_multisig_descriptor() {
        let pubkeys = vec![
            "xpub1...".to_string(),
            "xpub2...".to_string(),
            "xpub3...".to_string(),
        ];

        let desc =
            DescriptorWallet::create_multisig_descriptor(2, &pubkeys, Network::Bitcoin).unwrap();

        assert_eq!(desc.descriptor_type, DescriptorType::Multi);
        assert!(desc.descriptor.contains("multi(2,"));
    }

    #[test]
    fn test_invalid_multisig_threshold() {
        let pubkeys = vec!["xpub1...".to_string(), "xpub2...".to_string()];

        // Threshold too high
        let result = DescriptorWallet::create_multisig_descriptor(3, &pubkeys, Network::Bitcoin);
        assert!(result.is_err());

        // Threshold zero
        let result = DescriptorWallet::create_multisig_descriptor(0, &pubkeys, Network::Bitcoin);
        assert!(result.is_err());
    }

    #[test]
    fn test_descriptor_range() {
        let range = DescriptorRange::new(0, 9).unwrap();
        assert_eq!(range.count(), 10);

        let single = DescriptorRange::single(5);
        assert_eq!(single.count(), 1);

        let from_count = DescriptorRange::from_count(20);
        assert_eq!(from_count.start, 0);
        assert_eq!(from_count.end, 19);
    }

    #[test]
    fn test_invalid_range() {
        let result = DescriptorRange::new(10, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_descriptor_config_defaults() {
        let config = DescriptorConfig::default();
        assert_eq!(config.descriptor_type, DescriptorType::Wpkh);
        assert_eq!(config.network, Network::Bitcoin);
        assert!(config.validate_checksum);
    }
}
