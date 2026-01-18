//! PSBT (Partially Signed Bitcoin Transactions) support
//!
//! Provides functionality for:
//! - Creating unsigned transactions
//! - External signing support (hardware wallets)
//! - User BTC withdrawals
//! - Issuer revenue payouts

use bitcoin::consensus::encode;
use bitcoin::psbt::Psbt;
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::{BitcoinError, Result};

/// PSBT builder for creating unsigned transactions
pub struct PsbtBuilder {
    inputs: Vec<UtxoInput>,
    outputs: Vec<TxOutput>,
    change_address: Option<Address<bitcoin::address::NetworkUnchecked>>,
    fee_rate_sat_vb: u64,
    #[allow(dead_code)]
    network: Network,
}

/// UTXO input for transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoInput {
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
    pub script_pubkey: String,
    /// Optional redeem script for P2SH
    pub redeem_script: Option<String>,
    /// Optional witness script for P2WSH
    pub witness_script: Option<String>,
}

/// Transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub address: String,
    pub amount_sats: u64,
}

/// Result of PSBT creation
#[derive(Debug, Clone, Serialize)]
pub struct PsbtResult {
    /// Base64-encoded PSBT
    pub psbt_base64: String,
    /// Transaction ID (unsigned)
    pub unsigned_txid: String,
    /// Total input amount in sats
    pub total_input_sats: u64,
    /// Total output amount in sats
    pub total_output_sats: u64,
    /// Fee in sats
    pub fee_sats: u64,
    /// Fee rate in sat/vB
    pub fee_rate: u64,
    /// Virtual size in vBytes
    pub vsize: u64,
}

impl PsbtBuilder {
    /// Create a new PSBT builder
    pub fn new(network: Network) -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            change_address: None,
            fee_rate_sat_vb: 10, // Default 10 sat/vB
            network,
        }
    }

    /// Add an input UTXO
    pub fn add_input(mut self, input: UtxoInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Add multiple inputs
    pub fn add_inputs(mut self, inputs: Vec<UtxoInput>) -> Self {
        self.inputs.extend(inputs);
        self
    }

    /// Add an output
    pub fn add_output(mut self, output: TxOutput) -> Self {
        self.outputs.push(output);
        self
    }

    /// Add multiple outputs
    pub fn add_outputs(mut self, outputs: Vec<TxOutput>) -> Self {
        self.outputs.extend(outputs);
        self
    }

    /// Set the change address
    pub fn change_address(mut self, address: &str) -> Result<Self> {
        let addr = Address::from_str(address)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid change address: {}", e)))?;
        self.change_address = Some(addr);
        Ok(self)
    }

    /// Set the fee rate in sat/vB
    pub fn fee_rate(mut self, sat_per_vb: u64) -> Self {
        self.fee_rate_sat_vb = sat_per_vb;
        self
    }

    /// Build the PSBT
    pub fn build(self) -> Result<PsbtResult> {
        if self.inputs.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No inputs provided".to_string(),
            ));
        }

        if self.outputs.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No outputs provided".to_string(),
            ));
        }

        // Calculate total input amount
        let total_input: u64 = self.inputs.iter().map(|i| i.amount_sats).sum();

        // Calculate total output amount
        let total_output: u64 = self.outputs.iter().map(|o| o.amount_sats).sum();

        // Estimate transaction size for fee calculation
        // P2WPKH: ~68 vB per input, ~31 vB per output, ~11 vB overhead
        let estimated_vsize =
            11 + (self.inputs.len() as u64 * 68) + ((self.outputs.len() + 1) as u64 * 31);
        let estimated_fee = estimated_vsize * self.fee_rate_sat_vb;

        // Check if we have enough funds
        if total_input < total_output + estimated_fee {
            return Err(BitcoinError::InvalidTransaction(format!(
                "Insufficient funds: input {} < output {} + fee {}",
                total_input, total_output, estimated_fee
            )));
        }

        // Create transaction inputs
        let tx_inputs: Vec<TxIn> = self
            .inputs
            .iter()
            .map(|input| {
                let txid = Txid::from_str(&input.txid).map_err(|e| {
                    BitcoinError::InvalidTransaction(format!("Invalid txid: {}", e))
                })?;
                Ok(TxIn {
                    previous_output: OutPoint {
                        txid,
                        vout: input.vout,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Create transaction outputs
        let mut tx_outputs: Vec<TxOut> = self
            .outputs
            .iter()
            .map(|output| {
                let address = Address::from_str(&output.address)
                    .map_err(|e| {
                        BitcoinError::InvalidAddress(format!("Invalid output address: {}", e))
                    })?
                    .assume_checked();
                Ok(TxOut {
                    value: Amount::from_sat(output.amount_sats),
                    script_pubkey: address.script_pubkey(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Add change output if needed
        let change_amount = total_input - total_output - estimated_fee;
        if change_amount > 546 {
            // Dust threshold
            if let Some(change_addr) = self.change_address {
                tx_outputs.push(TxOut {
                    value: Amount::from_sat(change_amount),
                    script_pubkey: change_addr.assume_checked().script_pubkey(),
                });
            }
        }

        // Calculate final output total (including change) before moving tx_outputs
        let final_output_total: u64 = tx_outputs.iter().map(|o| o.value.to_sat()).sum();
        let actual_fee = total_input - final_output_total;

        // Create unsigned transaction
        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: tx_inputs,
            output: tx_outputs,
        };

        // Create PSBT
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx.clone()).map_err(|e| {
            BitcoinError::InvalidTransaction(format!("Failed to create PSBT: {}", e))
        })?;

        // Add input information
        for (i, input) in self.inputs.iter().enumerate() {
            let script_pubkey = ScriptBuf::from_hex(&input.script_pubkey).map_err(|e| {
                BitcoinError::InvalidTransaction(format!("Invalid script_pubkey: {}", e))
            })?;

            // Set witness UTXO for SegWit inputs
            psbt.inputs[i].witness_utxo = Some(TxOut {
                value: Amount::from_sat(input.amount_sats),
                script_pubkey,
            });

            // Add redeem script if provided (P2SH)
            if let Some(ref redeem) = input.redeem_script {
                let redeem_script = ScriptBuf::from_hex(redeem).map_err(|e| {
                    BitcoinError::InvalidTransaction(format!("Invalid redeem_script: {}", e))
                })?;
                psbt.inputs[i].redeem_script = Some(redeem_script);
            }

            // Add witness script if provided (P2WSH)
            if let Some(ref witness) = input.witness_script {
                let witness_script = ScriptBuf::from_hex(witness).map_err(|e| {
                    BitcoinError::InvalidTransaction(format!("Invalid witness_script: {}", e))
                })?;
                psbt.inputs[i].witness_script = Some(witness_script);
            }
        }

        // Serialize PSBT to base64
        let psbt_bytes = psbt.serialize();
        let psbt_base64 = base64_encode(&psbt_bytes);

        let vsize = unsigned_tx.vsize() as u64;

        Ok(PsbtResult {
            psbt_base64,
            unsigned_txid: unsigned_tx.compute_txid().to_string(),
            total_input_sats: total_input,
            total_output_sats: total_output,
            fee_sats: actual_fee,
            fee_rate: if vsize > 0 { actual_fee / vsize } else { 0 },
            vsize,
        })
    }
}

/// Simple base64 encoder
fn base64_encode(data: &[u8]) -> String {
    const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut i = 0;

    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }

    result
}

/// PSBT manager for handling withdrawals and payouts
pub struct PsbtManager {
    network: Network,
    default_fee_rate: u64,
}

impl PsbtManager {
    pub fn new(network: Network) -> Self {
        Self {
            network,
            default_fee_rate: 10,
        }
    }

    /// Set default fee rate
    pub fn with_fee_rate(mut self, sat_per_vb: u64) -> Self {
        self.default_fee_rate = sat_per_vb;
        self
    }

    /// Create a withdrawal PSBT
    pub fn create_withdrawal(
        &self,
        utxos: Vec<UtxoInput>,
        destination: &str,
        amount_sats: u64,
        change_address: &str,
    ) -> Result<PsbtResult> {
        PsbtBuilder::new(self.network)
            .add_inputs(utxos)
            .add_output(TxOutput {
                address: destination.to_string(),
                amount_sats,
            })
            .change_address(change_address)?
            .fee_rate(self.default_fee_rate)
            .build()
    }

    /// Create a batch withdrawal PSBT (multiple recipients)
    pub fn create_batch_withdrawal(
        &self,
        utxos: Vec<UtxoInput>,
        recipients: Vec<(String, u64)>,
        change_address: &str,
    ) -> Result<PsbtResult> {
        let outputs: Vec<TxOutput> = recipients
            .into_iter()
            .map(|(address, amount)| TxOutput {
                address,
                amount_sats: amount,
            })
            .collect();

        PsbtBuilder::new(self.network)
            .add_inputs(utxos)
            .add_outputs(outputs)
            .change_address(change_address)?
            .fee_rate(self.default_fee_rate)
            .build()
    }

    /// Create an issuer payout PSBT
    pub fn create_payout(
        &self,
        utxos: Vec<UtxoInput>,
        issuer_address: &str,
        amount_sats: u64,
        platform_address: &str,
        platform_fee_sats: u64,
        change_address: &str,
    ) -> Result<PsbtResult> {
        let mut outputs = vec![TxOutput {
            address: issuer_address.to_string(),
            amount_sats,
        }];

        if platform_fee_sats > 0 {
            outputs.push(TxOutput {
                address: platform_address.to_string(),
                amount_sats: platform_fee_sats,
            });
        }

        PsbtBuilder::new(self.network)
            .add_inputs(utxos)
            .add_outputs(outputs)
            .change_address(change_address)?
            .fee_rate(self.default_fee_rate)
            .build()
    }

    /// Combine multiple PSBTs (for multi-sig)
    pub fn combine_psbts(&self, psbts: Vec<&str>) -> Result<String> {
        if psbts.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No PSBTs provided".to_string(),
            ));
        }

        // Decode first PSBT
        let mut combined = self.decode_psbt(psbts[0])?;

        // Combine signatures from other PSBTs
        for psbt_str in psbts.iter().skip(1) {
            let other = self.decode_psbt(psbt_str)?;
            combined.combine(other).map_err(|e| {
                BitcoinError::InvalidTransaction(format!("Failed to combine: {}", e))
            })?;
        }

        // Re-encode
        let combined_bytes = combined.serialize();
        Ok(base64_encode(&combined_bytes))
    }

    /// Check if a PSBT is fully signed
    pub fn is_finalized(&self, psbt_base64: &str) -> Result<bool> {
        let psbt = self.decode_psbt(psbt_base64)?;

        // Check if all inputs have final script signatures or witnesses
        for input in &psbt.inputs {
            if input.final_script_sig.is_none() && input.final_script_witness.is_none() {
                // Check if it has partial signatures
                if input.partial_sigs.is_empty() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Finalize and extract the signed transaction
    pub fn finalize_and_extract(&self, psbt_base64: &str) -> Result<SignedTransaction> {
        let mut psbt = self.decode_psbt(psbt_base64)?;

        // Finalize all inputs
        for i in 0..psbt.inputs.len() {
            // Simple finalization for P2WPKH
            if let Some(ref witness_utxo) = psbt.inputs[i].witness_utxo {
                if witness_utxo.script_pubkey.is_p2wpkh() {
                    if let Some((&pubkey, sig)) = psbt.inputs[i].partial_sigs.iter().next() {
                        let mut witness = Witness::new();
                        witness.push(sig.to_vec());
                        witness.push(pubkey.to_bytes());
                        psbt.inputs[i].final_script_witness = Some(witness);
                        psbt.inputs[i].partial_sigs.clear();
                    }
                }
            }
        }

        // Extract the final transaction
        let tx = psbt
            .extract_tx()
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Failed to extract: {}", e)))?;

        let raw_hex = encode::serialize_hex(&tx);

        Ok(SignedTransaction {
            txid: tx.compute_txid().to_string(),
            raw_hex,
            vsize: tx.vsize() as u64,
            weight: tx.weight().to_wu(),
        })
    }

    /// Decode a base64 PSBT
    fn decode_psbt(&self, psbt_base64: &str) -> Result<Psbt> {
        let bytes = base64_decode(psbt_base64)?;
        Psbt::deserialize(&bytes)
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Invalid PSBT: {}", e)))
    }
}

/// Simple base64 decoder
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        if c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        }

        let value = BASE64_CHARS.iter().position(|&x| x == c).ok_or_else(|| {
            BitcoinError::InvalidTransaction(format!("Invalid base64 character: {}", c as char))
        })? as u32;

        buffer = (buffer << 6) | value;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

/// Signed transaction ready for broadcast
#[derive(Debug, Clone, Serialize)]
pub struct SignedTransaction {
    pub txid: String,
    pub raw_hex: String,
    pub vsize: u64,
    pub weight: u64,
}

/// Withdrawal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    pub user_id: Uuid,
    pub destination_address: String,
    pub amount_sats: u64,
    pub fee_rate: Option<u64>,
}

/// Withdrawal result
#[derive(Debug, Clone, Serialize)]
pub struct WithdrawalResult {
    pub withdrawal_id: Uuid,
    pub psbt: PsbtResult,
    pub status: WithdrawalStatus,
}

/// Withdrawal status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalStatus {
    /// PSBT created, awaiting signatures
    PendingSignature,
    /// Fully signed, ready to broadcast
    ReadyToBroadcast,
    /// Broadcast to network
    Broadcast,
    /// Confirmed on chain
    Confirmed,
    /// Failed
    Failed,
    /// Cancelled by user
    Cancelled,
}

/// Fee estimation result
#[derive(Debug, Clone, Serialize)]
pub struct FeeEstimation {
    /// Fee rate for fastest confirmation (1-2 blocks)
    pub fast_sat_vb: u64,
    /// Fee rate for medium priority (3-6 blocks)
    pub medium_sat_vb: u64,
    /// Fee rate for low priority (12+ blocks)
    pub slow_sat_vb: u64,
    /// Estimated fee for a typical 1-in-2-out transaction
    pub typical_tx_fee_sats: HashMap<String, u64>,
}

impl FeeEstimation {
    /// Create fee estimation from Bitcoin Core fee rates
    pub fn from_rates(fast: f64, medium: f64, slow: f64) -> Self {
        let fast_sat_vb = (fast * 100_000.0) as u64; // BTC/kB to sat/vB
        let medium_sat_vb = (medium * 100_000.0) as u64;
        let slow_sat_vb = (slow * 100_000.0) as u64;

        // Typical 1-in-2-out P2WPKH transaction is ~141 vB
        let typical_vsize = 141u64;

        let mut typical_fees = HashMap::new();
        typical_fees.insert("fast".to_string(), fast_sat_vb * typical_vsize);
        typical_fees.insert("medium".to_string(), medium_sat_vb * typical_vsize);
        typical_fees.insert("slow".to_string(), slow_sat_vb * typical_vsize);

        Self {
            fast_sat_vb,
            medium_sat_vb,
            slow_sat_vb,
            typical_tx_fee_sats: typical_fees,
        }
    }
}
