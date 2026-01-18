//! CoinJoin Implementation
//!
//! CoinJoin is a privacy technique where multiple users combine their
//! transactions into a single transaction, making it difficult for observers
//! to determine which inputs correspond to which outputs.
//!
//! This implementation provides basic CoinJoin coordination and participation.

use crate::error::BitcoinError;
use bitcoin::{Address, Amount, OutPoint, Transaction, TxIn, TxOut};
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

    /// Add a signature from a participant
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

        // Find participant
        let participant = self
            .participants
            .iter_mut()
            .find(|p| p.id == participant_id)
            .ok_or_else(|| BitcoinError::Validation("Participant not found".to_string()))?;

        participant.has_signed = true;
        self.signatures.insert(participant_id, signature);

        // Check if all participants have signed
        if self.participants.iter().all(|p| p.has_signed) {
            self.state = SessionState::Broadcasting;
        }

        Ok(())
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
}
