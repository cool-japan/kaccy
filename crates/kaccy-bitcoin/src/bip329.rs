//! BIP 329: Wallet Label Export Format
//!
//! This module implements BIP 329, which defines a standard format for exporting
//! and importing wallet labels. This allows users to backup their transaction labels
//! and address notes, and restore them across different wallet implementations.
//!
//! # Features
//!
//! - Label management for addresses and transactions
//! - JSON export/import format
//! - Label types (tx, addr, pubkey, input, output, xpub)
//! - Spendable flag for address privacy
//! - Reference field for linking related labels
//!
//! # Example
//!
//! ```rust
//! use kaccy_bitcoin::bip329::{LabelManager, LabelRecord, LabelType};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = LabelManager::new();
//!
//! // Add a label for an address
//! manager.add_address_label(
//!     "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
//!     "Donation to Alice",
//!     true,
//! )?;
//!
//! // Add a label for a transaction
//! manager.add_transaction_label(
//!     "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd",
//!     "Payment for services",
//! )?;
//!
//! // Export all labels to JSON
//! let json = manager.export_json()?;
//!
//! // Import labels from JSON
//! let imported_manager = LabelManager::from_json(&json)?;
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of label according to BIP 329
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelType {
    /// Transaction label
    Tx,
    /// Address label
    Addr,
    /// Public key label
    Pubkey,
    /// Input label (txid:vout)
    Input,
    /// Output label (txid:vout)
    Output,
    /// Extended public key label
    Xpub,
}

impl LabelType {
    /// Get the string representation
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tx => "tx",
            Self::Addr => "addr",
            Self::Pubkey => "pubkey",
            Self::Input => "input",
            Self::Output => "output",
            Self::Xpub => "xpub",
        }
    }
}

/// A label record according to BIP 329
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelRecord {
    /// Type of the labeled item
    #[serde(rename = "type")]
    pub label_type: LabelType,

    /// Reference identifier (address, txid, etc.)
    #[serde(rename = "ref")]
    pub reference: String,

    /// The label text
    pub label: String,

    /// Optional origin information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,

    /// Whether this address is spendable (for addr type only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spendable: Option<bool>,
}

impl LabelRecord {
    /// Create a new label record
    pub fn new(label_type: LabelType, reference: String, label: String) -> Self {
        Self {
            label_type,
            reference,
            label,
            origin: None,
            spendable: None,
        }
    }

    /// Create a transaction label
    pub fn transaction(txid: String, label: String) -> Self {
        Self::new(LabelType::Tx, txid, label)
    }

    /// Create an address label
    pub fn address(address: String, label: String, spendable: bool) -> Self {
        Self {
            label_type: LabelType::Addr,
            reference: address,
            label,
            origin: None,
            spendable: Some(spendable),
        }
    }

    /// Create a public key label
    pub fn pubkey(pubkey: String, label: String) -> Self {
        Self::new(LabelType::Pubkey, pubkey, label)
    }

    /// Create an input label (txid:vout format)
    pub fn input(txid: String, vout: u32, label: String) -> Self {
        Self::new(LabelType::Input, format!("{}:{}", txid, vout), label)
    }

    /// Create an output label (txid:vout format)
    pub fn output(txid: String, vout: u32, label: String) -> Self {
        Self::new(LabelType::Output, format!("{}:{}", txid, vout), label)
    }

    /// Create an xpub label
    pub fn xpub(xpub: String, label: String) -> Self {
        Self::new(LabelType::Xpub, xpub, label)
    }

    /// Set the origin field
    pub fn with_origin(mut self, origin: String) -> Self {
        self.origin = Some(origin);
        self
    }
}

/// Label manager for BIP 329 operations
#[derive(Debug, Clone)]
pub struct LabelManager {
    /// All label records indexed by type and reference
    labels: HashMap<String, LabelRecord>,
}

impl LabelManager {
    /// Create a new label manager
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
        }
    }

    /// Add a label record
    pub fn add_label(&mut self, record: LabelRecord) -> Result<(), BitcoinError> {
        let key = Self::make_key(&record.label_type, &record.reference);
        self.labels.insert(key, record);
        Ok(())
    }

    /// Add a transaction label
    pub fn add_transaction_label(&mut self, txid: &str, label: &str) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::transaction(
            txid.to_string(),
            label.to_string(),
        ))
    }

    /// Add an address label
    pub fn add_address_label(
        &mut self,
        address: &str,
        label: &str,
        spendable: bool,
    ) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::address(
            address.to_string(),
            label.to_string(),
            spendable,
        ))
    }

    /// Add a public key label
    pub fn add_pubkey_label(&mut self, pubkey: &str, label: &str) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::pubkey(pubkey.to_string(), label.to_string()))
    }

    /// Add an input label
    pub fn add_input_label(
        &mut self,
        txid: &str,
        vout: u32,
        label: &str,
    ) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::input(
            txid.to_string(),
            vout,
            label.to_string(),
        ))
    }

    /// Add an output label
    pub fn add_output_label(
        &mut self,
        txid: &str,
        vout: u32,
        label: &str,
    ) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::output(
            txid.to_string(),
            vout,
            label.to_string(),
        ))
    }

    /// Add an xpub label
    pub fn add_xpub_label(&mut self, xpub: &str, label: &str) -> Result<(), BitcoinError> {
        self.add_label(LabelRecord::xpub(xpub.to_string(), label.to_string()))
    }

    /// Get a label by type and reference
    pub fn get_label(&self, label_type: LabelType, reference: &str) -> Option<&LabelRecord> {
        let key = Self::make_key(&label_type, reference);
        self.labels.get(&key)
    }

    /// Get a transaction label
    pub fn get_transaction_label(&self, txid: &str) -> Option<&str> {
        self.get_label(LabelType::Tx, txid)
            .map(|record| record.label.as_str())
    }

    /// Get an address label
    pub fn get_address_label(&self, address: &str) -> Option<&str> {
        self.get_label(LabelType::Addr, address)
            .map(|record| record.label.as_str())
    }

    /// Remove a label
    pub fn remove_label(&mut self, label_type: LabelType, reference: &str) -> Option<LabelRecord> {
        let key = Self::make_key(&label_type, reference);
        self.labels.remove(&key)
    }

    /// Get all labels
    pub fn get_all_labels(&self) -> Vec<&LabelRecord> {
        self.labels.values().collect()
    }

    /// Get labels by type
    pub fn get_labels_by_type(&self, label_type: LabelType) -> Vec<&LabelRecord> {
        self.labels
            .values()
            .filter(|record| record.label_type == label_type)
            .collect()
    }

    /// Export all labels to JSON format (BIP 329 compliant)
    pub fn export_json(&self) -> Result<String, BitcoinError> {
        let mut records: Vec<&LabelRecord> = self.labels.values().collect();
        records.sort_by(|a, b| {
            a.label_type
                .as_str()
                .cmp(b.label_type.as_str())
                .then(a.reference.cmp(&b.reference))
        });

        serde_json::to_string_pretty(&records)
            .map_err(|e| BitcoinError::InvalidInput(format!("JSON serialization failed: {}", e)))
    }

    /// Import labels from JSON format (BIP 329 compliant)
    pub fn from_json(json: &str) -> Result<Self, BitcoinError> {
        let records: Vec<LabelRecord> = serde_json::from_str(json)
            .map_err(|e| BitcoinError::InvalidInput(format!("JSON parsing failed: {}", e)))?;

        let mut manager = Self::new();
        for record in records {
            manager.add_label(record)?;
        }

        Ok(manager)
    }

    /// Import and merge labels from JSON
    pub fn import_json(&mut self, json: &str) -> Result<usize, BitcoinError> {
        let records: Vec<LabelRecord> = serde_json::from_str(json)
            .map_err(|e| BitcoinError::InvalidInput(format!("JSON parsing failed: {}", e)))?;

        let count = records.len();
        for record in records {
            self.add_label(record)?;
        }

        Ok(count)
    }

    /// Clear all labels
    pub fn clear(&mut self) {
        self.labels.clear();
    }

    /// Get the number of labels
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Check if there are no labels
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Make a unique key for the labels map
    fn make_key(label_type: &LabelType, reference: &str) -> String {
        format!("{}:{}", label_type.as_str(), reference)
    }
}

impl Default for LabelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_type() {
        assert_eq!(LabelType::Tx.as_str(), "tx");
        assert_eq!(LabelType::Addr.as_str(), "addr");
        assert_eq!(LabelType::Pubkey.as_str(), "pubkey");
        assert_eq!(LabelType::Input.as_str(), "input");
        assert_eq!(LabelType::Output.as_str(), "output");
        assert_eq!(LabelType::Xpub.as_str(), "xpub");
    }

    #[test]
    fn test_label_record_creation() {
        let tx_label = LabelRecord::transaction(
            "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd".to_string(),
            "Payment for services".to_string(),
        );
        assert_eq!(tx_label.label_type, LabelType::Tx);
        assert_eq!(tx_label.label, "Payment for services");

        let addr_label = LabelRecord::address(
            "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
            "Donation address".to_string(),
            true,
        );
        assert_eq!(addr_label.label_type, LabelType::Addr);
        assert_eq!(addr_label.spendable, Some(true));
    }

    #[test]
    fn test_label_manager() {
        let mut manager = LabelManager::new();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());

        manager
            .add_transaction_label(
                "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd",
                "Test transaction",
            )
            .unwrap();

        assert_eq!(manager.len(), 1);
        assert!(!manager.is_empty());

        let label = manager.get_transaction_label(
            "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd",
        );
        assert_eq!(label, Some("Test transaction"));
    }

    #[test]
    fn test_json_export_import() {
        let mut manager = LabelManager::new();
        manager
            .add_transaction_label(
                "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd",
                "Payment",
            )
            .unwrap();
        manager
            .add_address_label(
                "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
                "Donation",
                true,
            )
            .unwrap();

        let json = manager.export_json().unwrap();
        assert!(json.contains("Payment"));
        assert!(json.contains("Donation"));

        let imported = LabelManager::from_json(&json).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(
            imported.get_transaction_label(
                "f91d0a8a78462bc59398f2c5d7a84fcff491c26ba54c4833478b202796c8aafd"
            ),
            Some("Payment")
        );
    }

    #[test]
    fn test_label_removal() {
        let mut manager = LabelManager::new();
        manager.add_transaction_label("abcd1234", "Test").unwrap();
        assert_eq!(manager.len(), 1);

        let removed = manager.remove_label(LabelType::Tx, "abcd1234");
        assert!(removed.is_some());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_input_output_labels() {
        let mut manager = LabelManager::new();
        manager
            .add_input_label("txid123", 0, "Input from Alice")
            .unwrap();
        manager
            .add_output_label("txid123", 1, "Output to Bob")
            .unwrap();

        assert_eq!(manager.len(), 2);

        let input_labels = manager.get_labels_by_type(LabelType::Input);
        assert_eq!(input_labels.len(), 1);
        assert_eq!(input_labels[0].label, "Input from Alice");

        let output_labels = manager.get_labels_by_type(LabelType::Output);
        assert_eq!(output_labels.len(), 1);
        assert_eq!(output_labels[0].label, "Output to Bob");
    }

    #[test]
    fn test_import_merge() {
        let mut manager1 = LabelManager::new();
        manager1.add_transaction_label("tx1", "Label 1").unwrap();

        let json = r#"[
            {
                "type": "tx",
                "ref": "tx2",
                "label": "Label 2"
            }
        ]"#;

        let count = manager1.import_json(json).unwrap();
        assert_eq!(count, 1);
        assert_eq!(manager1.len(), 2);
    }
}
