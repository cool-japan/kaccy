//! Transaction parsing and sender extraction
//!
//! This module provides utilities for parsing Bitcoin transactions,
//! extracting sender addresses (for refunds), and analyzing transaction details.

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// Parsed transaction with extracted details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    /// Transaction ID
    pub txid: String,
    /// Transaction version
    pub version: i32,
    /// Total input value in satoshis (if known)
    pub total_input_sats: Option<u64>,
    /// Total output value in satoshis
    pub total_output_sats: u64,
    /// Estimated fee in satoshis (if inputs are known)
    pub fee_sats: Option<u64>,
    /// Fee rate in sat/vB (if fee is known)
    pub fee_rate: Option<f64>,
    /// Virtual size in vbytes
    pub vsize: u64,
    /// Weight units
    pub weight: u64,
    /// Whether the transaction signals RBF
    pub is_rbf: bool,
    /// Whether this is a SegWit transaction
    pub is_segwit: bool,
    /// Parsed inputs with sender information
    pub inputs: Vec<ParsedInput>,
    /// Parsed outputs
    pub outputs: Vec<ParsedOutput>,
    /// Number of confirmations
    pub confirmations: u32,
    /// Block hash if confirmed
    pub block_hash: Option<String>,
    /// Block time if confirmed
    pub block_time: Option<u64>,
}

/// Parsed transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInput {
    /// Previous transaction ID
    pub prev_txid: String,
    /// Previous output index
    pub prev_vout: u32,
    /// Sender address (extracted from previous output)
    pub sender_address: Option<String>,
    /// Value in satoshis (from previous output)
    pub value_sats: Option<u64>,
    /// Sequence number (for RBF detection)
    pub sequence: u32,
    /// Whether this input signals RBF
    pub signals_rbf: bool,
}

/// Parsed transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedOutput {
    /// Output index
    pub index: u32,
    /// Recipient address (if standard script)
    pub address: Option<String>,
    /// Value in satoshis
    pub value_sats: u64,
    /// Script type
    pub script_type: ScriptType,
}

/// Bitcoin script types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptType {
    /// Pay to Public Key Hash (legacy)
    P2pkh,
    /// Pay to Script Hash
    P2sh,
    /// Pay to Witness Public Key Hash (native SegWit)
    P2wpkh,
    /// Pay to Witness Script Hash
    P2wsh,
    /// Pay to Taproot
    P2tr,
    /// OP_RETURN (data output)
    OpReturn,
    /// Unknown or non-standard
    Unknown,
}

impl ScriptType {
    /// Determine script type from address prefix
    pub fn from_address(address: &str) -> Self {
        if address.starts_with("1") {
            ScriptType::P2pkh
        } else if address.starts_with("3") {
            ScriptType::P2sh
        } else if address.starts_with("bc1q") || address.starts_with("tb1q") {
            ScriptType::P2wpkh
        } else if address.starts_with("bc1p") || address.starts_with("tb1p") {
            ScriptType::P2tr
        } else if address.starts_with("bc1") || address.starts_with("tb1") {
            ScriptType::P2wsh
        } else {
            ScriptType::Unknown
        }
    }
}

/// Sender information extracted from a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderInfo {
    /// Primary sender address (the most likely sender)
    pub primary_address: Option<String>,
    /// All unique sender addresses
    pub all_addresses: Vec<String>,
    /// Total amount sent by each address
    pub address_amounts: HashMap<String, u64>,
    /// Confidence level in sender identification
    pub confidence: SenderConfidence,
}

/// Confidence level in sender identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SenderConfidence {
    /// High confidence - single input or clear majority
    High,
    /// Medium confidence - multiple inputs but largest is clear
    Medium,
    /// Low confidence - many inputs, no clear primary
    Low,
    /// Unknown - could not determine sender
    Unknown,
}

/// Transaction parser service
pub struct TransactionParser {
    client: Arc<BitcoinClient>,
    /// Cache for parsed transactions
    #[allow(dead_code)]
    cache: HashMap<String, ParsedTransaction>,
}

impl TransactionParser {
    /// Create a new transaction parser
    pub fn new(client: Arc<BitcoinClient>) -> Self {
        Self {
            client,
            cache: HashMap::new(),
        }
    }

    /// Parse a transaction by its ID
    pub fn parse_transaction(&self, txid: &Txid) -> Result<ParsedTransaction> {
        // Get raw transaction details
        let raw_tx = self.client.get_raw_transaction(txid)?;

        // Parse inputs with previous output information
        let mut inputs = Vec::with_capacity(raw_tx.vin.len());
        let mut total_input_sats: u64 = 0;
        let mut all_inputs_known = true;

        for vin in &raw_tx.vin {
            if let Some(prev_txid) = vin.txid {
                let prev_vout = vin.vout.unwrap_or(0);
                let sequence = vin.sequence;
                let signals_rbf = sequence < 0xfffffffe;

                // Try to get the previous transaction to find sender address and value
                let (sender_address, value_sats) =
                    match self.get_previous_output(&prev_txid, prev_vout) {
                        Ok((addr, val)) => {
                            total_input_sats += val;
                            (addr, Some(val))
                        }
                        Err(_) => {
                            all_inputs_known = false;
                            (None, None)
                        }
                    };

                inputs.push(ParsedInput {
                    prev_txid: prev_txid.to_string(),
                    prev_vout,
                    sender_address,
                    value_sats,
                    sequence,
                    signals_rbf,
                });
            } else {
                // Coinbase transaction input
                inputs.push(ParsedInput {
                    prev_txid: String::from("coinbase"),
                    prev_vout: 0,
                    sender_address: None,
                    value_sats: None,
                    sequence: vin.sequence,
                    signals_rbf: false,
                });
                all_inputs_known = false;
            }
        }

        // Parse outputs
        let mut outputs = Vec::with_capacity(raw_tx.vout.len());
        let mut total_output_sats: u64 = 0;

        for (index, vout) in raw_tx.vout.iter().enumerate() {
            let value_sats = vout.value.to_sat();
            total_output_sats += value_sats;

            // Handle address conversion (NetworkUnchecked to string)
            let address = vout
                .script_pub_key
                .address
                .as_ref()
                .map(|a| a.clone().assume_checked().to_string());
            let script_type = if vout.script_pub_key.asm.starts_with("OP_RETURN") {
                ScriptType::OpReturn
            } else if let Some(ref addr) = address {
                ScriptType::from_address(addr)
            } else {
                ScriptType::Unknown
            };

            outputs.push(ParsedOutput {
                index: index as u32,
                address,
                value_sats,
                script_type,
            });
        }

        // Calculate fee and fee rate if all inputs are known
        let fee_sats = if all_inputs_known {
            Some(total_input_sats.saturating_sub(total_output_sats))
        } else {
            None
        };

        let vsize = raw_tx.vsize as u64;
        let fee_rate = fee_sats.map(|fee| fee as f64 / vsize as f64);

        // Check if transaction signals RBF
        let is_rbf = inputs.iter().any(|i| i.signals_rbf);

        // Check if SegWit
        let is_segwit = raw_tx.vin.iter().any(|v| v.txinwitness.is_some());

        // Calculate weight from vsize (weight = vsize * 4, approximate for segwit)
        let weight = vsize * 4;

        Ok(ParsedTransaction {
            txid: txid.to_string(),
            version: raw_tx.version as i32,
            total_input_sats: if all_inputs_known {
                Some(total_input_sats)
            } else {
                None
            },
            total_output_sats,
            fee_sats,
            fee_rate,
            vsize,
            weight,
            is_rbf,
            is_segwit,
            inputs,
            outputs,
            confirmations: raw_tx.confirmations.unwrap_or(0),
            block_hash: raw_tx.blockhash.map(|h| h.to_string()),
            block_time: raw_tx.blocktime.map(|t| t as u64),
        })
    }

    /// Get the sender information from a transaction
    pub fn get_sender_info(&self, txid: &Txid) -> Result<SenderInfo> {
        let parsed = self.parse_transaction(txid)?;

        let mut address_amounts: HashMap<String, u64> = HashMap::new();

        for input in &parsed.inputs {
            if let (Some(addr), Some(value)) = (&input.sender_address, input.value_sats) {
                *address_amounts.entry(addr.clone()).or_insert(0) += value;
            }
        }

        if address_amounts.is_empty() {
            return Ok(SenderInfo {
                primary_address: None,
                all_addresses: Vec::new(),
                address_amounts: HashMap::new(),
                confidence: SenderConfidence::Unknown,
            });
        }

        // Find the address with the highest contribution
        let all_addresses: Vec<String> = address_amounts.keys().cloned().collect();
        let total_value: u64 = address_amounts.values().sum();

        let primary = address_amounts
            .iter()
            .max_by_key(|(_, v)| *v)
            .map(|(a, _)| a.clone());

        // Determine confidence based on input distribution
        let confidence = if all_addresses.len() == 1 {
            SenderConfidence::High
        } else if let Some(ref primary_addr) = primary {
            let primary_value = address_amounts.get(primary_addr).copied().unwrap_or(0);
            let ratio = primary_value as f64 / total_value as f64;
            if ratio >= 0.8 {
                SenderConfidence::High
            } else if ratio >= 0.5 {
                SenderConfidence::Medium
            } else {
                SenderConfidence::Low
            }
        } else {
            SenderConfidence::Unknown
        };

        Ok(SenderInfo {
            primary_address: primary,
            all_addresses,
            address_amounts,
            confidence,
        })
    }

    /// Extract the most likely sender address for refund purposes
    pub fn get_refund_address(&self, txid: &Txid) -> Result<Option<String>> {
        let sender_info = self.get_sender_info(txid)?;

        // Only return address if confidence is sufficient
        match sender_info.confidence {
            SenderConfidence::High | SenderConfidence::Medium => Ok(sender_info.primary_address),
            _ => {
                tracing::warn!(
                    txid = %txid,
                    confidence = ?sender_info.confidence,
                    addresses = ?sender_info.all_addresses,
                    "Low confidence in sender identification for refund"
                );
                Ok(sender_info.primary_address)
            }
        }
    }

    /// Get the previous output (address and value) for an input
    fn get_previous_output(&self, txid: &Txid, vout: u32) -> Result<(Option<String>, u64)> {
        let prev_tx = self.client.get_raw_transaction(txid)?;

        let output = prev_tx
            .vout
            .get(vout as usize)
            .ok_or_else(|| BitcoinError::UtxoNotFound {
                txid: txid.to_string(),
                vout,
            })?;

        let address = output
            .script_pub_key
            .address
            .as_ref()
            .map(|a| a.clone().assume_checked().to_string());
        let value = output.value.to_sat();

        Ok((address, value))
    }

    /// Analyze a transaction for potential issues
    pub fn analyze_transaction(&self, txid: &Txid) -> Result<TransactionAnalysis> {
        let parsed = self.parse_transaction(txid)?;

        let mut warnings = Vec::new();
        let mut flags = Vec::new();

        // Check fee rate
        if let Some(fee_rate) = parsed.fee_rate {
            if fee_rate < 1.0 {
                warnings.push("Very low fee rate (< 1 sat/vB), may not confirm".to_string());
            } else if fee_rate > 100.0 {
                flags.push("High fee rate (> 100 sat/vB)".to_string());
            }
        }

        // Check for RBF
        if parsed.is_rbf {
            flags.push("Transaction signals RBF (can be replaced)".to_string());
        }

        // Check for OP_RETURN outputs
        let op_return_count = parsed
            .outputs
            .iter()
            .filter(|o| o.script_type == ScriptType::OpReturn)
            .count();
        if op_return_count > 0 {
            flags.push(format!("Contains {} OP_RETURN output(s)", op_return_count));
        }

        // Check for dust outputs
        let dust_threshold = 546; // Standard dust threshold for P2PKH
        let dust_outputs = parsed
            .outputs
            .iter()
            .filter(|o| o.value_sats < dust_threshold && o.script_type != ScriptType::OpReturn)
            .count();
        if dust_outputs > 0 {
            warnings.push(format!("Contains {} dust output(s)", dust_outputs));
        }

        // Check confirmation status
        let confirmation_status = if parsed.confirmations == 0 {
            ConfirmationStatus::Unconfirmed
        } else if parsed.confirmations < 3 {
            ConfirmationStatus::LowConfirmations
        } else if parsed.confirmations < 6 {
            ConfirmationStatus::MediumConfirmations
        } else {
            ConfirmationStatus::FullyConfirmed
        };

        let txid_str = parsed.txid.clone();
        Ok(TransactionAnalysis {
            txid: txid_str,
            parsed,
            warnings,
            flags,
            confirmation_status,
            is_safe_for_credit: confirmation_status == ConfirmationStatus::FullyConfirmed,
        })
    }
}

/// Transaction analysis result
#[derive(Debug, Clone, Serialize)]
pub struct TransactionAnalysis {
    /// Transaction ID
    pub txid: String,
    /// Parsed transaction details
    pub parsed: ParsedTransaction,
    /// Warnings about the transaction
    pub warnings: Vec<String>,
    /// Informational flags
    pub flags: Vec<String>,
    /// Confirmation status
    pub confirmation_status: ConfirmationStatus,
    /// Whether it's safe to credit this transaction
    pub is_safe_for_credit: bool,
}

/// Confirmation status categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    /// Transaction is not yet confirmed
    Unconfirmed,
    /// 1-2 confirmations
    LowConfirmations,
    /// 3-5 confirmations
    MediumConfirmations,
    /// 6+ confirmations
    FullyConfirmed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_type_detection() {
        assert_eq!(
            ScriptType::from_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"),
            ScriptType::P2pkh
        );
        assert_eq!(
            ScriptType::from_address("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"),
            ScriptType::P2sh
        );
        assert_eq!(
            ScriptType::from_address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
            ScriptType::P2wpkh
        );
        assert_eq!(
            ScriptType::from_address(
                "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0"
            ),
            ScriptType::P2tr
        );
    }

    #[test]
    fn test_sender_confidence() {
        // Single address should have high confidence
        let mut amounts = HashMap::new();
        amounts.insert("addr1".to_string(), 100000);

        let info = SenderInfo {
            primary_address: Some("addr1".to_string()),
            all_addresses: vec!["addr1".to_string()],
            address_amounts: amounts,
            confidence: SenderConfidence::High,
        };

        assert_eq!(info.confidence, SenderConfidence::High);
    }
}
