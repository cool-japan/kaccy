//! CoinJoin Implementation
//!
//! CoinJoin is a privacy technique where multiple users combine their
//! transactions into a single transaction, making it difficult for observers
//! to determine which inputs correspond to which outputs.
//!
//! This implementation provides basic CoinJoin coordination and participation.

use crate::error::BitcoinError;
use bitcoin::ecdsa;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::{
    Address, Amount, CompressedPublicKey, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Witness,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// CoinJoin session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Waiting for participants to register
    Registration,
    /// Collecting inputs from participants
    InputCollection,
    /// Collecting outputs from participants
    OutputCollection,
    /// Signing phase
    Signing,
    /// Broadcasting the transaction
    Broadcasting,
    /// Session completed successfully
    Completed,
    /// Session failed or cancelled
    Failed,
}

/// CoinJoin session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Minimum number of participants
    pub min_participants: usize,
    /// Maximum number of participants
    pub max_participants: usize,
    /// Standard denomination (in satoshis)
    pub denomination: u64,
    /// Coordinator fee per participant (in satoshis)
    pub coordinator_fee: u64,
    /// Mining fee per participant (in satoshis)
    pub mining_fee_per_participant: u64,
    /// Timeout for registration (seconds)
    pub registration_timeout: u64,
    /// Timeout for signing (seconds)
    pub signing_timeout: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            min_participants: 3,
            max_participants: 100,
            denomination: 100_000,           // 0.001 BTC
            coordinator_fee: 1000,           // 1000 sats
            mining_fee_per_participant: 500, // 500 sats
            registration_timeout: 600,       // 10 minutes
            signing_timeout: 300,            // 5 minutes
        }
    }
}

/// CoinJoin participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Unique participant ID
    pub id: Uuid,
    /// Input being contributed
    pub input: ParticipantInput,
    /// Output address to receive funds (as string)
    pub output_address: String,
    /// Change address (if needed, as string)
    pub change_address: Option<String>,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Whether the participant has signed
    pub has_signed: bool,
}

impl Participant {
    /// Get the output address as an Address type
    pub fn get_output_address(&self) -> Result<Address, BitcoinError> {
        Address::from_str(&self.output_address)
            .map_err(|e| BitcoinError::InvalidAddress(e.to_string()))
            .map(|a| a.assume_checked())
    }

    /// Get the change address as an Address type
    pub fn get_change_address(&self) -> Result<Option<Address>, BitcoinError> {
        self.change_address
            .as_ref()
            .map(|addr| {
                Address::from_str(addr)
                    .map_err(|e| BitcoinError::InvalidAddress(e.to_string()))
                    .map(|a| a.assume_checked())
            })
            .transpose()
    }
}

/// Input contributed by a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInput {
    /// Outpoint being spent
    pub outpoint: OutPoint,
    /// Value of the input (satoshis)
    pub value: u64,
    /// Script pubkey
    pub script_pubkey: Vec<u8>,
}

impl ParticipantInput {
    /// Parse the raw `script_pubkey` bytes into a typed [`ScriptBuf`].
    pub fn script_buf(&self) -> ScriptBuf {
        ScriptBuf::from_bytes(self.script_pubkey.clone())
    }

    /// Return the input's scriptPubKey, erroring unless it is a BIP141 P2WPKH
    /// (`OP_0 <20-byte-key-hash>`) program.
    ///
    /// CoinJoin signing in this module is implemented for native segwit v0
    /// P2WPKH inputs; non-P2WPKH inputs are rejected with an honest error
    /// rather than silently mis-signed.
    pub fn p2wpkh_script_pubkey(&self) -> Result<ScriptBuf, BitcoinError> {
        let script = self.script_buf();
        if !script.is_p2wpkh() {
            return Err(BitcoinError::InvalidInput(format!(
                "CoinJoin signing requires a P2WPKH input, got script {}",
                script.to_hex_string()
            )));
        }
        Ok(script)
    }
}

/// CoinJoin session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinJoinSession {
    /// Session ID
    pub id: Uuid,
    /// Session configuration
    pub config: SessionConfig,
    /// Current state
    pub state: SessionState,
    /// Participants
    pub participants: Vec<Participant>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Transaction being built
    pub transaction: Option<Transaction>,
    /// Signatures collected
    pub signatures: HashMap<Uuid, Vec<u8>>,
}

impl CoinJoinSession {
    /// Create a new CoinJoin session
    pub fn new(config: SessionConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            state: SessionState::Registration,
            participants: Vec::new(),
            created_at: Utc::now(),
            transaction: None,
            signatures: HashMap::new(),
        }
    }

    /// Add a participant to the session
    pub fn add_participant(
        &mut self,
        input: ParticipantInput,
        output_address: Address,
        change_address: Option<Address>,
    ) -> Result<Uuid, BitcoinError> {
        if self.state != SessionState::Registration {
            return Err(BitcoinError::Validation(
                "Session is not in registration state".to_string(),
            ));
        }

        if self.participants.len() >= self.config.max_participants {
            return Err(BitcoinError::Validation("Session is full".to_string()));
        }

        // Validate input value meets minimum requirements
        let required_input = self.config.denomination
            + self.config.coordinator_fee
            + self.config.mining_fee_per_participant;

        if input.value < required_input {
            return Err(BitcoinError::Validation(format!(
                "Input value {} is less than required {}",
                input.value, required_input
            )));
        }

        let participant = Participant {
            id: Uuid::new_v4(),
            input,
            output_address: output_address.to_string(),
            change_address: change_address.map(|a| a.to_string()),
            registered_at: Utc::now(),
            has_signed: false,
        };

        let participant_id = participant.id;
        self.participants.push(participant);

        // Check if we can move to next phase
        if self.participants.len() >= self.config.min_participants {
            self.state = SessionState::InputCollection;
        }

        Ok(participant_id)
    }

    /// Build the CoinJoin transaction
    pub fn build_transaction(&mut self) -> Result<(), BitcoinError> {
        if self.state != SessionState::InputCollection {
            return Err(BitcoinError::Validation(
                "Cannot build transaction in current state".to_string(),
            ));
        }

        if self.participants.len() < self.config.min_participants {
            return Err(BitcoinError::Validation(
                "Not enough participants".to_string(),
            ));
        }

        // Collect inputs
        let mut inputs = Vec::new();
        for participant in &self.participants {
            inputs.push(TxIn {
                previous_output: participant.input.outpoint,
                script_sig: bitcoin::blockdata::script::ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            });
        }

        // Collect outputs (equal denomination outputs + change)
        let mut outputs = Vec::new();

        // Add equal denomination outputs for each participant
        for participant in &self.participants {
            let addr = participant.get_output_address()?;
            outputs.push(TxOut {
                value: Amount::from_sat(self.config.denomination),
                script_pubkey: addr.script_pubkey(),
            });
        }

        // Add change outputs if needed
        for participant in &self.participants {
            let total_input = participant.input.value;
            let total_output = self.config.denomination
                + self.config.coordinator_fee
                + self.config.mining_fee_per_participant;

            let change = total_input.saturating_sub(total_output);
            if change > 0 {
                if let Some(change_addr) = participant.get_change_address()? {
                    outputs.push(TxOut {
                        value: Amount::from_sat(change),
                        script_pubkey: change_addr.script_pubkey(),
                    });
                }
            }
        }

        // Shuffle outputs for privacy
        // (In a real implementation, use a verifiable shuffle)

        let transaction = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::blockdata::locktime::absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        self.transaction = Some(transaction);
        self.state = SessionState::Signing;

        Ok(())
    }

    /// Locate a participant and its input index within the transaction.
    ///
    /// In [`build_transaction`](Self::build_transaction) inputs are appended in
    /// participant order, so the input index equals the participant's position
    /// in [`participants`](Self::participants).
    fn participant_input_index(&self, participant_id: Uuid) -> Result<usize, BitcoinError> {
        self.participants
            .iter()
            .position(|p| p.id == participant_id)
            .ok_or_else(|| BitcoinError::Validation("Participant not found".to_string()))
    }

    /// Compute the BIP143 segwit-v0 P2WPKH sighash (SIGHASH_ALL) that the
    /// participant at `participant_id` must sign for their own input.
    ///
    /// The transaction must already have been assembled via
    /// [`build_transaction`](Self::build_transaction). Each participant signs
    /// only their own input, committing (via SIGHASH_ALL) to the full set of
    /// CoinJoin inputs and outputs.
    pub fn participant_sighash(&self, participant_id: Uuid) -> Result<[u8; 32], BitcoinError> {
        let input_index = self.participant_input_index(participant_id)?;
        let transaction = self.transaction.as_ref().ok_or_else(|| {
            BitcoinError::Validation("Transaction has not been built yet".to_string())
        })?;
        let participant = &self.participants[input_index];
        let script_pubkey = participant.input.p2wpkh_script_pubkey()?;
        let value = Amount::from_sat(participant.input.value);

        let mut cache = SighashCache::new(transaction);
        let sighash = cache
            .p2wpkh_signature_hash(input_index, &script_pubkey, value, EcdsaSighashType::All)
            .map_err(|e| {
                BitcoinError::Validation(format!("Failed to compute P2WPKH sighash: {e}"))
            })?;

        Ok(sighash.to_byte_array())
    }

    /// Produce a real signature/witness for a participant's own P2WPKH input.
    ///
    /// This computes the correct BIP143 sighash (SIGHASH_ALL), signs it with the
    /// participant's `secp256k1` secret key, and returns the consensus-encoded
    /// P2WPKH witness stack `[<DER signature || SIGHASH_ALL>, <compressed pubkey>]`.
    ///
    /// The secret key must control the input's scriptPubKey: the derived public
    /// key's witness-program hash is checked against `OP_0 <hash>`, so signing a
    /// foreign input fails loudly instead of yielding a bogus witness.
    ///
    /// The returned bytes are exactly what [`add_signature`](Self::add_signature)
    /// validates and stores.
    pub fn sign_input(
        &self,
        participant_id: Uuid,
        secret_key: &SecretKey,
    ) -> Result<Vec<u8>, BitcoinError> {
        let input_index = self.participant_input_index(participant_id)?;
        let participant = &self.participants[input_index];
        let script_pubkey = participant.input.p2wpkh_script_pubkey()?;

        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, secret_key);
        let compressed = CompressedPublicKey(public_key);

        // The signing key must actually control this input.
        let expected = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash());
        if expected != script_pubkey {
            return Err(BitcoinError::Validation(
                "Secret key does not control this participant's input (key-hash mismatch)"
                    .to_string(),
            ));
        }

        let sighash = self.participant_sighash(participant_id)?;
        let message = Message::from_digest(sighash);
        let signature = secp.sign_ecdsa(&message, secret_key);
        let ecdsa_sig = ecdsa::Signature::sighash_all(signature);

        let witness = Witness::p2wpkh(&ecdsa_sig, &public_key);
        Ok(bitcoin::consensus::encode::serialize(&witness))
    }

    /// Validate and store a participant's signature.
    ///
    /// `signature` must be the consensus-encoded P2WPKH witness stack
    /// `[<DER signature || sighash flag>, <compressed pubkey>]` (exactly what
    /// [`sign_input`](Self::sign_input) returns). Before storing, this method:
    ///
    /// 1. recomputes the participant's own BIP143 sighash;
    /// 2. checks the supplied pubkey's witness-program hash matches the input's
    ///    `OP_0 <hash>` scriptPubKey; and
    /// 3. verifies the ECDSA signature against the sighash and pubkey.
    ///
    /// Invalid or mismatched signatures are rejected and never stored, so the
    /// session can never advance to broadcasting with a bogus witness.
    pub fn add_signature(
        &mut self,
        participant_id: Uuid,
        signature: Vec<u8>,
    ) -> Result<(), BitcoinError> {
        if self.state != SessionState::Signing {
            return Err(BitcoinError::Validation(
                "Session is not in signing state".to_string(),
            ));
        }

        // Validate the witness against this participant's input *before* storing.
        self.verify_participant_witness(participant_id, &signature)?;

        if let Some(participant) = self
            .participants
            .iter_mut()
            .find(|p| p.id == participant_id)
        {
            participant.has_signed = true;
        }
        self.signatures.insert(participant_id, signature);

        // Check if all participants have signed
        if self.participants.iter().all(|p| p.has_signed) {
            self.state = SessionState::Broadcasting;
        }

        Ok(())
    }

    /// Decode and cryptographically verify a P2WPKH witness for `participant_id`.
    ///
    /// Returns the parsed `(signature, public_key)` on success and an honest
    /// error on any malformed, mismatched, or invalid witness.
    fn verify_participant_witness(
        &self,
        participant_id: Uuid,
        witness_bytes: &[u8],
    ) -> Result<(ecdsa::Signature, PublicKey), BitcoinError> {
        let input_index = self.participant_input_index(participant_id)?;
        let participant = &self.participants[input_index];
        let script_pubkey = participant.input.p2wpkh_script_pubkey()?;

        let witness: Witness = bitcoin::consensus::encode::deserialize(witness_bytes)
            .map_err(|e| BitcoinError::Validation(format!("Malformed witness encoding: {e}")))?;
        if witness.len() != 2 {
            return Err(BitcoinError::Validation(format!(
                "P2WPKH witness must have 2 elements, found {}",
                witness.len()
            )));
        }

        let sig_bytes = witness
            .nth(0)
            .ok_or_else(|| BitcoinError::Validation("Missing signature element".to_string()))?;
        let pubkey_bytes = witness
            .nth(1)
            .ok_or_else(|| BitcoinError::Validation("Missing pubkey element".to_string()))?;

        let ecdsa_sig = ecdsa::Signature::from_slice(sig_bytes)
            .map_err(|e| BitcoinError::Validation(format!("Invalid ECDSA signature: {e}")))?;
        if ecdsa_sig.sighash_type != EcdsaSighashType::All {
            return Err(BitcoinError::Validation(format!(
                "Unexpected sighash type {:?}; CoinJoin requires SIGHASH_ALL",
                ecdsa_sig.sighash_type
            )));
        }

        let compressed = CompressedPublicKey::from_slice(pubkey_bytes)
            .map_err(|e| BitcoinError::Validation(format!("Invalid public key: {e}")))?;

        // The pubkey must hash to the input's witness program.
        let expected = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash());
        if expected != script_pubkey {
            return Err(BitcoinError::Validation(
                "Public key does not match this participant's input (key-hash mismatch)"
                    .to_string(),
            ));
        }

        // Verify the signature against the recomputed sighash.
        let sighash = self.participant_sighash(participant_id)?;
        let message = Message::from_digest(sighash);
        let secp = Secp256k1::verification_only();
        secp.verify_ecdsa(&message, &ecdsa_sig.signature, &compressed.0)
            .map_err(|e| {
                BitcoinError::Validation(format!("Signature verification failed: {e}"))
            })?;

        Ok((ecdsa_sig, compressed.0))
    }

    /// Finalize the CoinJoin: assemble every participant's verified witness into
    /// the transaction and return the fully-signed, broadcastable result.
    ///
    /// Every input is re-verified against its participant's sighash and pubkey
    /// while the witness is installed, so a transaction is only ever returned
    /// when *all* inputs carry a valid signature. If any required signature is
    /// missing this returns an honest error — it never emits an
    /// unsigned-but-claimed-complete transaction.
    pub fn finalize(&self) -> Result<Transaction, BitcoinError> {
        let base_tx = self.transaction.as_ref().ok_or_else(|| {
            BitcoinError::Validation("Transaction has not been built yet".to_string())
        })?;

        if base_tx.input.len() != self.participants.len() {
            return Err(BitcoinError::Validation(format!(
                "Transaction input count {} does not match participant count {}",
                base_tx.input.len(),
                self.participants.len()
            )));
        }

        let mut tx = base_tx.clone();

        for (input_index, participant) in self.participants.iter().enumerate() {
            let witness_bytes = self.signatures.get(&participant.id).ok_or_else(|| {
                BitcoinError::Validation(format!(
                    "Missing signature for participant {} (input {input_index})",
                    participant.id
                ))
            })?;

            // Re-verify before installing the witness, then decode the canonical
            // witness stack to attach to the transaction.
            self.verify_participant_witness(participant.id, witness_bytes)?;
            let witness: Witness = bitcoin::consensus::encode::deserialize(witness_bytes)
                .map_err(|e| {
                    BitcoinError::Validation(format!("Malformed stored witness: {e}"))
                })?;
            tx.input[input_index].witness = witness;
        }

        Ok(tx)
    }

    /// Get the number of signatures collected
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Check if session is ready to broadcast
    pub fn is_ready_to_broadcast(&self) -> bool {
        self.state == SessionState::Broadcasting && self.signatures.len() == self.participants.len()
    }

    /// Mark session as completed
    pub fn complete(&mut self) {
        self.state = SessionState::Completed;
    }

    /// Mark session as failed
    pub fn fail(&mut self) {
        self.state = SessionState::Failed;
    }
}

/// CoinJoin coordinator - manages sessions
pub struct CoinJoinCoordinator {
    /// Active sessions
    sessions: HashMap<Uuid, CoinJoinSession>,
    /// Default configuration
    default_config: SessionConfig,
}

impl CoinJoinCoordinator {
    /// Create a new coordinator
    pub fn new(default_config: SessionConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            default_config,
        }
    }

    /// Create a new session
    pub fn create_session(&mut self, config: Option<SessionConfig>) -> Uuid {
        let session = CoinJoinSession::new(config.unwrap_or_else(|| self.default_config.clone()));
        let session_id = session.id;
        self.sessions.insert(session_id, session);
        session_id
    }

    /// Get a session
    pub fn get_session(&self, session_id: &Uuid) -> Option<&CoinJoinSession> {
        self.sessions.get(session_id)
    }

    /// Get a mutable session
    pub fn get_session_mut(&mut self, session_id: &Uuid) -> Option<&mut CoinJoinSession> {
        self.sessions.get_mut(session_id)
    }

    /// Join a session as a participant
    pub fn join_session(
        &mut self,
        session_id: Uuid,
        input: ParticipantInput,
        output_address: Address,
        change_address: Option<Address>,
    ) -> Result<Uuid, BitcoinError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| BitcoinError::Validation("Session not found".to_string()))?;

        session.add_participant(input, output_address, change_address)
    }

    /// List active sessions
    pub fn list_active_sessions(&self) -> Vec<&CoinJoinSession> {
        self.sessions
            .values()
            .filter(|s| {
                matches!(
                    s.state,
                    SessionState::Registration
                        | SessionState::InputCollection
                        | SessionState::OutputCollection
                        | SessionState::Signing
                )
            })
            .collect()
    }

    /// Clean up old sessions
    pub fn cleanup_old_sessions(&mut self, max_age_secs: i64) {
        let now = Utc::now();
        self.sessions.retain(|_, session| {
            let age = now.signed_duration_since(session.created_at).num_seconds();
            age < max_age_secs || session.state == SessionState::Broadcasting
        });
    }
}

/// CoinJoin participant client
pub struct CoinJoinClient {
    /// Participant ID (if registered)
    participant_id: Option<Uuid>,
}

impl CoinJoinClient {
    /// Create a new client
    pub fn new() -> Self {
        Self {
            participant_id: None,
        }
    }

    /// Register for a CoinJoin session
    pub fn register(
        &mut self,
        coordinator: &mut CoinJoinCoordinator,
        session_id: Uuid,
        input: ParticipantInput,
        output_address: Address,
        change_address: Option<Address>,
    ) -> Result<Uuid, BitcoinError> {
        let participant_id =
            coordinator.join_session(session_id, input, output_address, change_address)?;

        self.participant_id = Some(participant_id);
        Ok(participant_id)
    }

    /// Submit a signature
    pub fn submit_signature(
        &self,
        coordinator: &mut CoinJoinCoordinator,
        session_id: Uuid,
        signature: Vec<u8>,
    ) -> Result<(), BitcoinError> {
        let participant_id = self
            .participant_id
            .ok_or_else(|| BitcoinError::Validation("Not registered".to_string()))?;

        let session = coordinator
            .get_session_mut(&session_id)
            .ok_or_else(|| BitcoinError::Validation("Session not found".to_string()))?;

        session.add_signature(participant_id, signature)
    }
}

impl Default for CoinJoinClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;
    use bitcoin::hashes::Hash;
    use std::str::FromStr;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = CoinJoinSession::new(config);

        assert_eq!(session.state, SessionState::Registration);
        assert_eq!(session.participants.len(), 0);
    }

    #[test]
    fn test_add_participant() {
        let mut session = CoinJoinSession::new(SessionConfig::default());
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let input = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 200_000,
            script_pubkey: vec![],
        };

        let result = session.add_participant(input, address, None);
        assert!(result.is_ok());
        assert_eq!(session.participants.len(), 1);
    }

    #[test]
    fn test_coordinator() {
        let mut coordinator = CoinJoinCoordinator::new(SessionConfig::default());
        let session_id = coordinator.create_session(None);

        assert!(coordinator.get_session(&session_id).is_some());
    }

    #[test]
    fn test_session_state_progression() {
        let mut session = CoinJoinSession::new(SessionConfig {
            min_participants: 2,
            ..Default::default()
        });

        assert_eq!(session.state, SessionState::Registration);

        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        // Add first participant
        let input1 = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 200_000,
            script_pubkey: vec![],
        };
        session
            .add_participant(input1, address.clone(), None)
            .unwrap();

        // Still in registration
        assert_eq!(session.state, SessionState::Registration);

        // Add second participant
        let input2 = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 1,
            },
            value: 200_000,
            script_pubkey: vec![],
        };
        session.add_participant(input2, address, None).unwrap();

        // Should move to input collection
        assert_eq!(session.state, SessionState::InputCollection);
    }

    #[test]
    fn test_insufficient_input_value() {
        let mut session = CoinJoinSession::new(SessionConfig::default());
        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let input = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 1000, // Too small
            script_pubkey: vec![],
        };

        let result = session.add_participant(input, address, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_coinjoin_client() {
        let mut client = CoinJoinClient::new();
        let mut coordinator = CoinJoinCoordinator::new(SessionConfig::default());
        let session_id = coordinator.create_session(None);

        let address = Address::from_str("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
            .unwrap()
            .assume_checked();

        let input = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 200_000,
            script_pubkey: vec![],
        };

        let result = client.register(&mut coordinator, session_id, input, address, None);
        assert!(result.is_ok());
    }

    // ---- Real signing / finalization tests ----

    use bitcoin::Network;
    use bitcoin::secp256k1::SecretKey;

    /// Deterministic 32-byte secret key from a single seed byte (non-zero).
    fn secret_key_from_seed(seed: u8) -> SecretKey {
        let mut bytes = [seed; 32];
        // Guarantee a valid, non-zero scalar below the curve order.
        bytes[0] = 0x01;
        SecretKey::from_slice(&bytes).expect("valid secret key")
    }

    /// Build a P2WPKH (input scriptPubKey, receiving address) controlled by `sk`.
    fn p2wpkh_for(sk: &SecretKey) -> (ScriptBuf, Address) {
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, sk);
        let compressed = CompressedPublicKey(pk);
        let script_pubkey = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash());
        let address = Address::p2wpkh(&compressed, Network::Regtest);
        (script_pubkey, address)
    }

    /// Construct a 2-participant session already advanced to the signing phase,
    /// returning the session plus each participant's id and secret key.
    fn signing_ready_session() -> (CoinJoinSession, [(Uuid, SecretKey); 2]) {
        let config = SessionConfig {
            min_participants: 2,
            max_participants: 2,
            denomination: 100_000,
            coordinator_fee: 1000,
            mining_fee_per_participant: 500,
            ..Default::default()
        };
        let mut session = CoinJoinSession::new(config);

        let sk_a = secret_key_from_seed(0x11);
        let sk_b = secret_key_from_seed(0x22);
        let (spk_a, addr_a) = p2wpkh_for(&sk_a);
        let (spk_b, addr_b) = p2wpkh_for(&sk_b);

        let input_a = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 200_000,
            script_pubkey: spk_a.to_bytes(),
        };
        let input_b = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 1,
            },
            value: 200_000,
            script_pubkey: spk_b.to_bytes(),
        };

        let id_a = session.add_participant(input_a, addr_a, None).unwrap();
        let id_b = session.add_participant(input_b, addr_b, None).unwrap();
        assert_eq!(session.state, SessionState::InputCollection);

        session.build_transaction().unwrap();
        assert_eq!(session.state, SessionState::Signing);

        (session, [(id_a, sk_a), (id_b, sk_b)])
    }

    /// Independently re-verify a finalized transaction: each input must carry a
    /// 2-element P2WPKH witness whose signature validates against the recomputed
    /// BIP143 sighash and whose pubkey matches the prevout's witness program.
    fn assert_all_inputs_signed(session: &CoinJoinSession, tx: &Transaction) {
        let secp = Secp256k1::verification_only();
        assert_eq!(tx.input.len(), session.participants.len());

        for (index, participant) in session.participants.iter().enumerate() {
            let witness = &tx.input[index].witness;
            assert_eq!(witness.len(), 2, "input {index} must have a P2WPKH witness");

            let sig_bytes = witness.nth(0).unwrap();
            let pubkey_bytes = witness.nth(1).unwrap();
            let ecdsa_sig = ecdsa::Signature::from_slice(sig_bytes).unwrap();
            assert_eq!(ecdsa_sig.sighash_type, EcdsaSighashType::All);
            let compressed = CompressedPublicKey::from_slice(pubkey_bytes).unwrap();

            // Pubkey must match the input's witness program.
            let expected = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash());
            assert_eq!(expected, participant.input.p2wpkh_script_pubkey().unwrap());

            // Recompute the sighash from the finalized tx and verify the signature.
            let value = Amount::from_sat(participant.input.value);
            let mut cache = SighashCache::new(tx);
            let sighash = cache
                .p2wpkh_signature_hash(
                    index,
                    &participant.input.p2wpkh_script_pubkey().unwrap(),
                    value,
                    EcdsaSighashType::All,
                )
                .unwrap();
            let message = Message::from_digest(sighash.to_byte_array());
            secp.verify_ecdsa(&message, &ecdsa_sig.signature, &compressed.0)
                .expect("each finalized input signature must verify");
        }
    }

    #[test]
    fn test_real_two_party_coinjoin_signs_and_finalizes() {
        let (mut session, parties) = signing_ready_session();

        // Each participant signs only their own input and submits the witness.
        for (id, sk) in &parties {
            let witness = session.sign_input(*id, sk).unwrap();
            session.add_signature(*id, witness).unwrap();
        }

        // All signatures collected -> ready to broadcast.
        assert_eq!(session.state, SessionState::Broadcasting);
        assert!(session.is_ready_to_broadcast());
        assert_eq!(session.signature_count(), 2);

        // Finalize and independently verify every input.
        let tx = session.finalize().unwrap();
        assert_all_inputs_signed(&session, &tx);

        // A finalized P2WPKH tx must report a non-trivial weight/vsize.
        assert!(tx.weight().to_wu() > 0);
    }

    #[test]
    fn test_sign_input_rejects_foreign_key() {
        let (session, parties) = signing_ready_session();
        let (id_a, _sk_a) = &parties[0];
        let (_id_b, sk_b) = &parties[1];

        // Signing participant A's input with participant B's key must fail.
        let result = session.sign_input(*id_a, sk_b);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_signature_rejects_invalid_witness() {
        let (mut session, parties) = signing_ready_session();
        let (id_a, _sk_a) = &parties[0];

        // Garbage bytes are not a valid witness.
        let bad = session.add_signature(*id_a, vec![0u8; 8]);
        assert!(bad.is_err());
        assert!(!session.participants[0].has_signed);
        assert_eq!(session.signature_count(), 0);
        assert_eq!(session.state, SessionState::Signing);
    }

    #[test]
    fn test_add_signature_rejects_wrong_key_signature() {
        let (mut session, parties) = signing_ready_session();
        let (id_a, _sk_a) = &parties[0];
        let (_id_b, sk_b) = &parties[1];

        // A structurally valid witness signed by the *wrong* key (B's key over
        // A's sighash) must be rejected: pubkey-hash mismatch with A's input.
        let secp = Secp256k1::new();
        let pk_b = PublicKey::from_secret_key(&secp, sk_b);
        let sighash = session.participant_sighash(*id_a).unwrap();
        let message = Message::from_digest(sighash);
        let sig = secp.sign_ecdsa(&message, sk_b);
        let ecdsa_sig = ecdsa::Signature::sighash_all(sig);
        let forged = bitcoin::consensus::encode::serialize(&Witness::p2wpkh(&ecdsa_sig, &pk_b));

        let result = session.add_signature(*id_a, forged);
        assert!(result.is_err());
        assert!(!session.participants[0].has_signed);
    }

    #[test]
    fn test_finalize_fails_with_missing_signature() {
        let (mut session, parties) = signing_ready_session();
        let (id_a, sk_a) = &parties[0];

        // Only participant A signs; B's signature is missing.
        let witness_a = session.sign_input(*id_a, sk_a).unwrap();
        session.add_signature(*id_a, witness_a).unwrap();

        // Session has NOT advanced to broadcasting (B still unsigned).
        assert_eq!(session.state, SessionState::Signing);

        // Finalizing must return an honest error, never a partial tx.
        let result = session.finalize();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Missing signature"), "unexpected error: {msg}");
    }

    #[test]
    fn test_add_signature_rejected_outside_signing_state() {
        let (session, parties) = signing_ready_session();
        // Fresh session still in registration.
        let mut fresh = CoinJoinSession::new(SessionConfig {
            min_participants: 2,
            ..Default::default()
        });
        let (id_a, sk_a) = &parties[0];
        let witness = session.sign_input(*id_a, sk_a).unwrap();

        let result = fresh.add_signature(*id_a, witness);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_p2wpkh_input_signing_errors() {
        let mut session = CoinJoinSession::new(SessionConfig {
            min_participants: 2,
            max_participants: 2,
            ..Default::default()
        });
        let sk_a = secret_key_from_seed(0x33);
        let sk_b = secret_key_from_seed(0x44);
        let (_spk_a, addr_a) = p2wpkh_for(&sk_a);
        let (spk_b, addr_b) = p2wpkh_for(&sk_b);

        // Participant A's input uses a non-P2WPKH (P2PKH) scriptPubKey.
        let secp = Secp256k1::new();
        let pk_a = PublicKey::from_secret_key(&secp, &sk_a);
        let p2pkh = ScriptBuf::new_p2pkh(&CompressedPublicKey(pk_a).pubkey_hash());

        let input_a = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            value: 200_000,
            script_pubkey: p2pkh.to_bytes(),
        };
        let input_b = ParticipantInput {
            outpoint: OutPoint {
                txid: Txid::all_zeros(),
                vout: 1,
            },
            value: 200_000,
            script_pubkey: spk_b.to_bytes(),
        };

        let id_a = session.add_participant(input_a, addr_a, None).unwrap();
        let _id_b = session.add_participant(input_b, addr_b, None).unwrap();
        session.build_transaction().unwrap();

        // Signing the non-P2WPKH input must produce an honest error.
        let result = session.sign_input(id_a, &sk_a);
        assert!(result.is_err());
    }
}
