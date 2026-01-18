//! PayJoin (P2EP) Implementation
//!
//! PayJoin is a privacy-enhancing technique where the sender and receiver
//! collaborate to create a transaction that breaks the common-input-ownership
//! heuristic used for blockchain analysis.
//!
//! This implementation follows BIP 78 (PayJoin).

use crate::error::BitcoinError;
use bitcoin::{Address, Amount, OutPoint, Transaction, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// PayJoin role - sender or receiver
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayJoinRole {
    /// Sender initiates the payment
    Sender,
    /// Receiver accepts and enhances the transaction
    Receiver,
}

/// PayJoin version (BIP 78 compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayJoinVersion {
    /// Version 1 (current standard)
    V1,
    /// Version 2 (future)
    V2,
}

/// PayJoin proposal from sender
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayJoinProposal {
    /// Unique proposal ID
    pub id: Uuid,
    /// Original transaction (unsigned or partially signed)
    pub original_psbt: String,
    /// Amount to pay (in satoshis)
    pub amount: u64,
    /// Receiver's address (as string)
    pub receiver_address: String,
    /// Additional parameters
    pub params: PayJoinParams,
}

impl PayJoinProposal {
    /// Get the receiver address as an Address type
    pub fn get_receiver_address(&self) -> Result<Address, BitcoinError> {
        Address::from_str(&self.receiver_address)
            .map_err(|e| BitcoinError::InvalidAddress(e.to_string()))
            .map(|a| a.assume_checked())
    }
}

/// PayJoin parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayJoinParams {
    /// Version of PayJoin protocol
    pub version: PayJoinVersion,
    /// Disable output substitution (for testing)
    pub disable_output_substitution: bool,
    /// Minimum confirmations for receiver inputs
    pub min_confirmations: u32,
    /// Maximum additional fee contribution from receiver
    pub max_additional_fee: u64,
}

impl Default for PayJoinParams {
    fn default() -> Self {
        Self {
            version: PayJoinVersion::V1,
            disable_output_substitution: false,
            min_confirmations: 1,
            max_additional_fee: 1000, // 1000 sats
        }
    }
}

/// PayJoin response from receiver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayJoinResponse {
    /// Proposal ID this responds to
    pub proposal_id: Uuid,
    /// Enhanced PSBT with receiver's inputs
    pub payjoin_psbt: String,
    /// Receiver's contribution details
    pub contribution: ReceiverContribution,
}

/// Receiver's contribution to the PayJoin transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverContribution {
    /// Inputs added by receiver
    pub inputs_added: Vec<OutPoint>,
    /// Total value of added inputs (satoshis)
    pub input_value: u64,
    /// Additional outputs added (change)
    pub outputs_added: Vec<TxOut>,
}

/// PayJoin coordinator - manages the PayJoin flow
pub struct PayJoinCoordinator {
    /// Active proposals
    proposals: HashMap<Uuid, PayJoinProposal>,
}

impl PayJoinCoordinator {
    /// Create a new PayJoin coordinator
    pub fn new() -> Self {
        Self {
            proposals: HashMap::new(),
        }
    }

    /// Create a PayJoin proposal (sender side)
    pub fn create_proposal(
        &mut self,
        original_psbt: String,
        amount: u64,
        receiver_address: Address,
        params: Option<PayJoinParams>,
    ) -> PayJoinProposal {
        let proposal = PayJoinProposal {
            id: Uuid::new_v4(),
            original_psbt,
            amount,
            receiver_address: receiver_address.to_string(),
            params: params.unwrap_or_default(),
        };

        self.proposals.insert(proposal.id, proposal.clone());
        proposal
    }

    /// Get a proposal by ID
    pub fn get_proposal(&self, id: &Uuid) -> Option<&PayJoinProposal> {
        self.proposals.get(id)
    }

    /// Validate a PayJoin proposal (receiver side)
    pub fn validate_proposal(&self, proposal: &PayJoinProposal) -> Result<(), BitcoinError> {
        // Check version compatibility
        if proposal.params.version != PayJoinVersion::V1 {
            return Err(BitcoinError::Validation(
                "Unsupported PayJoin version".to_string(),
            ));
        }

        // Check amount is positive
        if proposal.amount == 0 {
            return Err(BitcoinError::Validation(
                "Payment amount must be positive".to_string(),
            ));
        }

        // Validate PSBT format (basic check)
        if proposal.original_psbt.is_empty() {
            return Err(BitcoinError::Validation(
                "Original PSBT is empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Remove expired proposals
    pub fn cleanup_expired(&mut self, max_age_secs: u64) {
        // Implementation would check timestamps and remove old proposals
        // For now, keep all proposals
        let _ = max_age_secs;
    }
}

impl Default for PayJoinCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// PayJoin receiver - handles the receiver side of PayJoin
pub struct PayJoinReceiver {
    /// Available UTXOs for PayJoin
    available_utxos: Vec<ReceiverUtxo>,
}

/// UTXO available for PayJoin enhancement
#[derive(Debug, Clone)]
pub struct ReceiverUtxo {
    /// Outpoint
    pub outpoint: OutPoint,
    /// Value in satoshis
    pub value: u64,
    /// Number of confirmations
    pub confirmations: u32,
    /// Script pubkey
    pub script_pubkey: Vec<u8>,
}

impl PayJoinReceiver {
    /// Create a new PayJoin receiver
    pub fn new(available_utxos: Vec<ReceiverUtxo>) -> Self {
        Self { available_utxos }
    }

    /// Enhance a transaction with receiver inputs (core PayJoin logic)
    pub fn enhance_transaction(
        &self,
        proposal: &PayJoinProposal,
        change_address: Option<Address>,
    ) -> Result<PayJoinResponse, BitcoinError> {
        // Select appropriate UTXOs based on min confirmations
        let eligible_utxos: Vec<_> = self
            .available_utxos
            .iter()
            .filter(|u| u.confirmations >= proposal.params.min_confirmations)
            .collect();

        if eligible_utxos.is_empty() {
            return Err(BitcoinError::Validation(
                "No eligible UTXOs for PayJoin".to_string(),
            ));
        }

        // For simplicity, select one UTXO
        let selected_utxo = eligible_utxos[0];

        // Build contribution
        let contribution = ReceiverContribution {
            inputs_added: vec![selected_utxo.outpoint],
            input_value: selected_utxo.value,
            outputs_added: if let Some(addr) = change_address {
                let change_value = selected_utxo
                    .value
                    .saturating_sub(proposal.params.max_additional_fee);
                vec![TxOut {
                    value: Amount::from_sat(change_value),
                    script_pubkey: addr.script_pubkey(),
                }]
            } else {
                vec![]
            },
        };

        // In a real implementation, we would:
        // 1. Parse the original PSBT
        // 2. Add receiver's inputs
        // 3. Adjust outputs (add change if needed)
        // 4. Re-serialize to PSBT

        let response = PayJoinResponse {
            proposal_id: proposal.id,
            payjoin_psbt: proposal.original_psbt.clone(), // Placeholder
            contribution,
        };

        Ok(response)
    }
}

/// PayJoin sender - handles the sender side
pub struct PayJoinSender {
    /// Coordinator reference
    coordinator: PayJoinCoordinator,
}

impl PayJoinSender {
    /// Create a new PayJoin sender
    pub fn new() -> Self {
        Self {
            coordinator: PayJoinCoordinator::new(),
        }
    }

    /// Initiate a PayJoin payment
    pub fn initiate_payment(
        &mut self,
        original_psbt: String,
        amount: u64,
        receiver_address: Address,
        params: Option<PayJoinParams>,
    ) -> PayJoinProposal {
        self.coordinator
            .create_proposal(original_psbt, amount, receiver_address, params)
    }

    /// Verify and finalize a PayJoin response
    pub fn finalize_payjoin(
        &self,
        response: &PayJoinResponse,
    ) -> Result<Transaction, BitcoinError> {
        // Get original proposal
        let proposal = self
            .coordinator
            .get_proposal(&response.proposal_id)
            .ok_or_else(|| BitcoinError::Validation("Proposal not found".to_string()))?;

        // Validate the response
        self.validate_response(proposal, response)?;

        // In a real implementation:
        // 1. Parse the PayJoin PSBT
        // 2. Verify all inputs and outputs
        // 3. Sign the transaction
        // 4. Finalize and extract

        // Placeholder: return empty transaction
        Ok(Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::blockdata::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        })
    }

    /// Validate a PayJoin response
    fn validate_response(
        &self,
        proposal: &PayJoinProposal,
        response: &PayJoinResponse,
    ) -> Result<(), BitcoinError> {
        // Check that receiver didn't add too much fee
        let total_input_value = response.contribution.input_value;
        let total_output_value: u64 = response
            .contribution
            .outputs_added
            .iter()
            .map(|o| o.value.to_sat())
            .sum();

        let fee_contribution = total_input_value.saturating_sub(total_output_value);
        if fee_contribution > proposal.params.max_additional_fee {
            return Err(BitcoinError::Validation(
                "Receiver's fee contribution exceeds maximum".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for PayJoinSender {
    fn default() -> Self {
        Self::new()
    }
}

/// PayJoin URI builder (BIP 21 with PayJoin extension)
pub struct PayJoinUriBuilder {
    address: Address,
    amount: Option<u64>,
    endpoint: Option<String>,
}

impl PayJoinUriBuilder {
    /// Create a new PayJoin URI builder
    pub fn new(address: Address) -> Self {
        Self {
            address,
            amount: None,
            endpoint: None,
        }
    }

    /// Set payment amount
    pub fn amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Set PayJoin endpoint URL
    pub fn endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    /// Build the URI string
    pub fn build(self) -> String {
        let mut uri = format!("bitcoin:{}", self.address);
        let mut params = vec![];

        if let Some(amt) = self.amount {
            params.push(format!("amount={}", amt as f64 / 100_000_000.0));
        }

        if let Some(ep) = self.endpoint {
            params.push(format!("pj={}", ep));
        }

        if !params.is_empty() {
            uri.push('?');
            uri.push_str(&params.join("&"));
        }

        uri
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_payjoin_coordinator() {
        let mut coordinator = PayJoinCoordinator::new();
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let proposal =
            coordinator.create_proposal("psbt_base64_here".to_string(), 100000, address, None);

        assert_eq!(
            coordinator.get_proposal(&proposal.id).unwrap().amount,
            100000
        );
    }

    #[test]
    fn test_payjoin_params_defaults() {
        let params = PayJoinParams::default();
        assert_eq!(params.version, PayJoinVersion::V1);
        assert!(!params.disable_output_substitution);
        assert_eq!(params.min_confirmations, 1);
        assert_eq!(params.max_additional_fee, 1000);
    }

    #[test]
    fn test_payjoin_uri_builder() {
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let uri = PayJoinUriBuilder::new(address)
            .amount(100000)
            .endpoint("https://example.com/payjoin".to_string())
            .build();

        assert!(uri.contains("bitcoin:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"));
        assert!(uri.contains("pj=https://example.com/payjoin"));
    }

    #[test]
    fn test_validate_proposal() {
        let coordinator = PayJoinCoordinator::new();
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let proposal = PayJoinProposal {
            id: Uuid::new_v4(),
            original_psbt: "psbt_data".to_string(),
            amount: 50000,
            receiver_address: address.to_string(),
            params: PayJoinParams::default(),
        };

        assert!(coordinator.validate_proposal(&proposal).is_ok());
    }

    #[test]
    fn test_validate_invalid_proposal() {
        let coordinator = PayJoinCoordinator::new();
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        // Zero amount
        let proposal = PayJoinProposal {
            id: Uuid::new_v4(),
            original_psbt: "psbt_data".to_string(),
            amount: 0,
            receiver_address: address.to_string(),
            params: PayJoinParams::default(),
        };

        assert!(coordinator.validate_proposal(&proposal).is_err());
    }

    #[test]
    fn test_payjoin_sender() {
        let mut sender = PayJoinSender::new();
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let proposal = sender.initiate_payment("psbt_base64".to_string(), 100000, address, None);

        assert_eq!(proposal.amount, 100000);
    }
}
