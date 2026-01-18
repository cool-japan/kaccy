//! Interoperability Protocols
//!
//! Provides comprehensive cross-chain interoperability support including:
//! - IBC (Inter-Blockchain Communication) integration
//! - Cross-chain message passing
//! - State verification
//! - Light client support

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

/// Re-export Chain from atomic_swaps
pub use super::atomic_swaps::Chain;

/// IBC channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelState {
    /// Channel is being initialized
    Init,
    /// Channel is trying to open
    TryOpen,
    /// Channel is open and ready for packets
    Open,
    /// Channel is closing
    Closing,
    /// Channel is closed
    Closed,
}

/// IBC channel for cross-chain communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IBCChannel {
    /// Channel ID
    pub id: String,
    /// Source chain
    pub source_chain: Chain,
    /// Destination chain
    pub destination_chain: Chain,
    /// Channel state
    pub state: ChannelState,
    /// Connection ID
    pub connection_id: String,
    /// Port ID (e.g., "transfer", "custom")
    pub port_id: String,
    /// Counterparty channel ID
    pub counterparty_channel_id: String,
    /// Version (e.g., "ics20-1" for token transfers)
    pub version: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl IBCChannel {
    /// Create a new IBC channel
    pub fn new(
        id: String,
        source_chain: Chain,
        destination_chain: Chain,
        connection_id: String,
        port_id: String,
        version: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            source_chain,
            destination_chain,
            state: ChannelState::Init,
            connection_id,
            port_id,
            counterparty_channel_id: String::new(),
            version,
            created_at: now,
            updated_at: now,
        }
    }

    /// Open the channel
    pub fn open(&mut self, counterparty_channel_id: String) -> Result<()> {
        if self.state != ChannelState::TryOpen {
            return Err(CoreError::Validation(format!(
                "Cannot open channel in state {:?}",
                self.state
            )));
        }
        self.state = ChannelState::Open;
        self.counterparty_channel_id = counterparty_channel_id;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Close the channel
    pub fn close(&mut self) -> Result<()> {
        if self.state != ChannelState::Open {
            return Err(CoreError::Validation(format!(
                "Cannot close channel in state {:?}",
                self.state
            )));
        }
        self.state = ChannelState::Closing;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Check if channel is open
    pub fn is_open(&self) -> bool {
        self.state == ChannelState::Open
    }
}

/// IBC packet for cross-chain message passing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IBCPacket {
    /// Packet sequence number
    pub sequence: u64,
    /// Source channel ID
    pub source_channel: String,
    /// Destination channel ID
    pub destination_channel: String,
    /// Packet data (serialized)
    pub data: Vec<u8>,
    /// Timeout height (block height)
    pub timeout_height: Option<u64>,
    /// Timeout timestamp
    pub timeout_timestamp: Option<DateTime<Utc>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl IBCPacket {
    /// Create a new IBC packet
    pub fn new(
        sequence: u64,
        source_channel: String,
        destination_channel: String,
        data: Vec<u8>,
        timeout_height: Option<u64>,
        timeout_timestamp: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            sequence,
            source_channel,
            destination_channel,
            data,
            timeout_height,
            timeout_timestamp,
            created_at: Utc::now(),
        }
    }

    /// Check if packet has timed out
    pub fn is_timed_out(&self, current_height: u64, current_time: DateTime<Utc>) -> bool {
        if let Some(timeout_height) = self.timeout_height {
            if current_height >= timeout_height {
                return true;
            }
        }
        if let Some(timeout_timestamp) = self.timeout_timestamp {
            if current_time >= timeout_timestamp {
                return true;
            }
        }
        false
    }

    /// Get packet hash for commitment
    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.source_channel.as_bytes());
        hasher.update(self.destination_channel.as_bytes());
        hasher.update(&self.data);
        if let Some(height) = self.timeout_height {
            hasher.update(height.to_le_bytes());
        }
        if let Some(timestamp) = self.timeout_timestamp {
            hasher.update(timestamp.timestamp().to_le_bytes());
        }
        hasher.finalize().to_vec()
    }
}

/// Cross-chain message for generic data transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainMessage {
    /// Message ID
    pub id: String,
    /// Source chain
    pub source_chain: Chain,
    /// Destination chain
    pub destination_chain: Chain,
    /// Sender address
    pub sender: String,
    /// Receiver address
    pub receiver: String,
    /// Message type (e.g., "token_transfer", "contract_call")
    pub message_type: String,
    /// Message payload (serialized)
    pub payload: Vec<u8>,
    /// Nonce for ordering
    pub nonce: u64,
    /// Gas limit for execution
    pub gas_limit: u64,
    /// Status
    pub status: MessageStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Message status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Message is pending
    Pending,
    /// Message is being relayed
    Relaying,
    /// Message is being executed on destination
    Executing,
    /// Message has been successfully delivered
    Delivered,
    /// Message has failed
    Failed,
    /// Message has timed out
    TimedOut,
}

impl CrossChainMessage {
    /// Create a new cross-chain message
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        source_chain: Chain,
        destination_chain: Chain,
        sender: String,
        receiver: String,
        message_type: String,
        payload: Vec<u8>,
        nonce: u64,
        gas_limit: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            source_chain,
            destination_chain,
            sender,
            receiver,
            message_type,
            payload,
            nonce,
            gas_limit,
            status: MessageStatus::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    /// Mark message as relaying
    pub fn mark_relaying(&mut self) {
        self.status = MessageStatus::Relaying;
        self.updated_at = Utc::now();
    }

    /// Mark message as executing
    pub fn mark_executing(&mut self) {
        self.status = MessageStatus::Executing;
        self.updated_at = Utc::now();
    }

    /// Mark message as delivered
    pub fn mark_delivered(&mut self) {
        self.status = MessageStatus::Delivered;
        self.updated_at = Utc::now();
    }

    /// Mark message as failed
    pub fn mark_failed(&mut self) {
        self.status = MessageStatus::Failed;
        self.updated_at = Utc::now();
    }

    /// Get message hash
    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.nonce.to_le_bytes());
        hasher.update(&self.payload);
        hasher.finalize().to_vec()
    }
}

/// State proof for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProof {
    /// Block height at which the state was captured
    pub height: u64,
    /// State root hash
    pub state_root: Vec<u8>,
    /// Merkle proof path
    pub proof: Vec<Vec<u8>>,
    /// Proven value
    pub value: Vec<u8>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl StateProof {
    /// Create a new state proof
    pub fn new(height: u64, state_root: Vec<u8>, proof: Vec<Vec<u8>>, value: Vec<u8>) -> Self {
        Self {
            height,
            state_root,
            proof,
            value,
            timestamp: Utc::now(),
        }
    }

    /// Verify the Merkle proof
    pub fn verify(&self, key: &[u8]) -> Result<bool> {
        let mut current_hash = self.compute_leaf_hash(key, &self.value);

        for sibling in &self.proof {
            current_hash = self.compute_internal_hash(&current_hash, sibling);
        }

        Ok(current_hash == self.state_root)
    }

    fn compute_leaf_hash(&self, key: &[u8], value: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(b"leaf:");
        hasher.update(key);
        hasher.update(value);
        hasher.finalize().to_vec()
    }

    fn compute_internal_hash(&self, left: &[u8], right: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(b"node:");
        if left <= right {
            hasher.update(left);
            hasher.update(right);
        } else {
            hasher.update(right);
            hasher.update(left);
        }
        hasher.finalize().to_vec()
    }
}

/// Light client header for another chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClientHeader {
    /// Chain this header belongs to
    pub chain: Chain,
    /// Block height
    pub height: u64,
    /// Block hash
    pub block_hash: Vec<u8>,
    /// Parent block hash
    pub parent_hash: Vec<u8>,
    /// State root
    pub state_root: Vec<u8>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Validator set hash
    pub validator_set_hash: Vec<u8>,
    /// Signatures from validators
    pub signatures: Vec<ValidatorSignature>,
}

/// Validator signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSignature {
    /// Validator address
    pub validator: String,
    /// Signature bytes
    pub signature: Vec<u8>,
    /// Voting power
    pub voting_power: u64,
}

impl LightClientHeader {
    /// Create a new light client header
    pub fn new(
        chain: Chain,
        height: u64,
        block_hash: Vec<u8>,
        parent_hash: Vec<u8>,
        state_root: Vec<u8>,
        timestamp: DateTime<Utc>,
        validator_set_hash: Vec<u8>,
    ) -> Self {
        Self {
            chain,
            height,
            block_hash,
            parent_hash,
            state_root,
            timestamp,
            validator_set_hash,
            signatures: Vec::new(),
        }
    }

    /// Add a validator signature
    pub fn add_signature(&mut self, signature: ValidatorSignature) {
        self.signatures.push(signature);
    }

    /// Get total voting power from signatures
    pub fn total_voting_power(&self) -> u64 {
        self.signatures.iter().map(|s| s.voting_power).sum()
    }

    /// Verify signatures meet threshold (e.g., 2/3+)
    pub fn verify_threshold(
        &self,
        total_validator_power: u64,
        threshold_numerator: u64,
        threshold_denominator: u64,
    ) -> bool {
        let signed_power = self.total_voting_power();
        signed_power * threshold_denominator >= total_validator_power * threshold_numerator
    }

    /// Get header hash
    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.height.to_le_bytes());
        hasher.update(&self.block_hash);
        hasher.update(&self.parent_hash);
        hasher.update(&self.state_root);
        hasher.update(self.timestamp.timestamp().to_le_bytes());
        hasher.update(&self.validator_set_hash);
        hasher.finalize().to_vec()
    }
}

/// Light client for tracking another chain's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClient {
    /// Tracked chain
    pub chain: Chain,
    /// Latest verified height
    pub latest_height: u64,
    /// Trusted headers (height -> header)
    pub trusted_headers: HashMap<u64, LightClientHeader>,
    /// Maximum age for headers (in seconds)
    pub max_header_age_seconds: i64,
    /// Trusting period (in seconds)
    pub trusting_period_seconds: i64,
}

impl LightClient {
    /// Create a new light client
    pub fn new(chain: Chain, trusting_period_seconds: i64) -> Self {
        Self {
            chain,
            latest_height: 0,
            trusted_headers: HashMap::new(),
            max_header_age_seconds: trusting_period_seconds,
            trusting_period_seconds,
        }
    }

    /// Update with a new verified header
    pub fn update(&mut self, header: LightClientHeader) -> Result<()> {
        // Check that header is for the correct chain
        if header.chain != self.chain {
            return Err(CoreError::Validation(format!(
                "Header is for chain {:?}, expected {:?}",
                header.chain, self.chain
            )));
        }

        // Check that header is newer than latest
        if header.height <= self.latest_height {
            return Err(CoreError::Validation(format!(
                "Header height {} is not newer than latest height {}",
                header.height, self.latest_height
            )));
        }

        // Check header age
        let now = Utc::now();
        let age_seconds = (now - header.timestamp).num_seconds();
        if age_seconds > self.max_header_age_seconds {
            return Err(CoreError::Validation(format!(
                "Header is too old: {} seconds > {} max",
                age_seconds, self.max_header_age_seconds
            )));
        }

        // Check parent hash matches if we have the parent
        if let Some(parent) = self.trusted_headers.get(&(header.height - 1)) {
            if parent.block_hash != header.parent_hash {
                return Err(CoreError::Validation("Parent hash mismatch".to_string()));
            }
        }

        // Store header and update latest height
        self.latest_height = header.height;
        self.trusted_headers.insert(header.height, header);

        // Prune old headers outside trusting period
        self.prune_old_headers(now);

        Ok(())
    }

    /// Get a trusted header at a specific height
    pub fn get_header(&self, height: u64) -> Option<&LightClientHeader> {
        self.trusted_headers.get(&height)
    }

    /// Verify a state proof against a trusted header
    pub fn verify_state(&self, height: u64, proof: &StateProof, key: &[u8]) -> Result<bool> {
        // Get the trusted header at this height
        let header = self.get_header(height).ok_or_else(|| {
            CoreError::Validation(format!("No trusted header at height {}", height))
        })?;

        // Verify proof height matches
        if proof.height != height {
            return Err(CoreError::Validation(format!(
                "Proof height {} doesn't match requested height {}",
                proof.height, height
            )));
        }

        // Verify state root matches
        if proof.state_root != header.state_root {
            return Err(CoreError::Validation("State root mismatch".to_string()));
        }

        // Verify the Merkle proof
        proof.verify(key)
    }

    /// Prune headers older than trusting period
    fn prune_old_headers(&mut self, now: DateTime<Utc>) {
        let cutoff_timestamp = now - chrono::Duration::seconds(self.trusting_period_seconds);
        self.trusted_headers
            .retain(|_, header| header.timestamp >= cutoff_timestamp);
    }
}

/// IBC connection manager
#[derive(Debug)]
pub struct IBCConnectionManager {
    /// Active channels (channel_id -> channel)
    pub channels: HashMap<String, IBCChannel>,
    /// Packet sequence counter per channel
    pub packet_sequences: HashMap<String, u64>,
    /// Pending packets
    pub pending_packets: Vec<IBCPacket>,
}

impl IBCConnectionManager {
    /// Create a new IBC connection manager
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            packet_sequences: HashMap::new(),
            pending_packets: Vec::new(),
        }
    }

    /// Register a new channel
    pub fn register_channel(&mut self, channel: IBCChannel) -> Result<()> {
        if self.channels.contains_key(&channel.id) {
            return Err(CoreError::Validation(format!(
                "Channel {} already exists",
                channel.id
            )));
        }
        self.packet_sequences.insert(channel.id.clone(), 0);
        self.channels.insert(channel.id.clone(), channel);
        Ok(())
    }

    /// Transition channel to TryOpen state (part of IBC handshake)
    pub fn try_open_channel(&mut self, channel_id: &str) -> Result<()> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or_else(|| CoreError::Validation(format!("Channel {} not found", channel_id)))?;
        if channel.state != ChannelState::Init {
            return Err(CoreError::Validation(format!(
                "Cannot try_open channel in state {:?}",
                channel.state
            )));
        }
        channel.state = ChannelState::TryOpen;
        channel.updated_at = Utc::now();
        Ok(())
    }

    /// Open a channel
    pub fn open_channel(
        &mut self,
        channel_id: &str,
        counterparty_channel_id: String,
    ) -> Result<()> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or_else(|| CoreError::Validation(format!("Channel {} not found", channel_id)))?;
        channel.open(counterparty_channel_id)?;
        Ok(())
    }

    /// Send a packet through a channel
    pub fn send_packet(
        &mut self,
        channel_id: &str,
        data: Vec<u8>,
        timeout_height: Option<u64>,
        timeout_timestamp: Option<DateTime<Utc>>,
    ) -> Result<IBCPacket> {
        let channel = self
            .channels
            .get(channel_id)
            .ok_or_else(|| CoreError::Validation(format!("Channel {} not found", channel_id)))?;

        if !channel.is_open() {
            return Err(CoreError::Validation(format!(
                "Channel {} is not open",
                channel_id
            )));
        }

        // Get next sequence number
        let sequence = self.packet_sequences.get_mut(channel_id).unwrap();
        *sequence += 1;

        let packet = IBCPacket::new(
            *sequence,
            channel.id.clone(),
            channel.counterparty_channel_id.clone(),
            data,
            timeout_height,
            timeout_timestamp,
        );

        self.pending_packets.push(packet.clone());

        Ok(packet)
    }

    /// Receive and acknowledge a packet
    pub fn receive_packet(
        &mut self,
        packet: &IBCPacket,
        current_height: u64,
        current_time: DateTime<Utc>,
    ) -> Result<()> {
        // Check timeout
        if packet.is_timed_out(current_height, current_time) {
            return Err(CoreError::Validation("Packet has timed out".to_string()));
        }

        // Remove from pending if it exists
        self.pending_packets
            .retain(|p| p.sequence != packet.sequence || p.source_channel != packet.source_channel);

        Ok(())
    }

    /// Get channel by ID
    pub fn get_channel(&self, channel_id: &str) -> Option<&IBCChannel> {
        self.channels.get(channel_id)
    }

    /// Get all channels between two chains
    pub fn get_channels_between(&self, source: Chain, destination: Chain) -> Vec<&IBCChannel> {
        self.channels
            .values()
            .filter(|c| c.source_chain == source && c.destination_chain == destination)
            .collect()
    }
}

impl Default for IBCConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ibc_channel_lifecycle() {
        let mut channel = IBCChannel::new(
            "channel-0".to_string(),
            Chain::Bitcoin,
            Chain::Ethereum,
            "connection-0".to_string(),
            "transfer".to_string(),
            "ics20-1".to_string(),
        );

        assert_eq!(channel.state, ChannelState::Init);
        assert!(!channel.is_open());

        channel.state = ChannelState::TryOpen;
        channel.open("channel-1".to_string()).unwrap();
        assert!(channel.is_open());

        channel.close().unwrap();
        assert_eq!(channel.state, ChannelState::Closing);
    }

    #[test]
    fn test_ibc_packet_timeout() {
        let timeout_height = Some(100);
        let timeout_timestamp = Some(Utc::now() + chrono::Duration::seconds(3600));

        let packet = IBCPacket::new(
            1,
            "channel-0".to_string(),
            "channel-1".to_string(),
            vec![1, 2, 3],
            timeout_height,
            timeout_timestamp,
        );

        // Should not be timed out
        assert!(!packet.is_timed_out(50, Utc::now()));

        // Should be timed out by height
        assert!(packet.is_timed_out(100, Utc::now()));

        // Should be timed out by timestamp
        assert!(packet.is_timed_out(50, Utc::now() + chrono::Duration::seconds(7200)));
    }

    #[test]
    fn test_cross_chain_message() {
        let mut message = CrossChainMessage::new(
            "msg-1".to_string(),
            Chain::Bitcoin,
            Chain::Ethereum,
            "sender".to_string(),
            "receiver".to_string(),
            "token_transfer".to_string(),
            vec![1, 2, 3],
            1,
            100000,
        );

        assert_eq!(message.status, MessageStatus::Pending);

        message.mark_relaying();
        assert_eq!(message.status, MessageStatus::Relaying);

        message.mark_executing();
        assert_eq!(message.status, MessageStatus::Executing);

        message.mark_delivered();
        assert_eq!(message.status, MessageStatus::Delivered);
    }

    #[test]
    fn test_light_client() {
        let mut client = LightClient::new(Chain::Bitcoin, 86400);

        let header = LightClientHeader::new(
            Chain::Bitcoin,
            100,
            vec![1, 2, 3],
            vec![0, 1, 2],
            vec![3, 4, 5],
            Utc::now(),
            vec![6, 7, 8],
        );

        client.update(header.clone()).unwrap();
        assert_eq!(client.latest_height, 100);

        let retrieved = client.get_header(100).unwrap();
        assert_eq!(retrieved.height, 100);
    }

    #[test]
    fn test_ibc_connection_manager() {
        let mut manager = IBCConnectionManager::new();

        let channel = IBCChannel::new(
            "channel-0".to_string(),
            Chain::Bitcoin,
            Chain::Ethereum,
            "connection-0".to_string(),
            "transfer".to_string(),
            "ics20-1".to_string(),
        );

        manager.register_channel(channel).unwrap();
        assert!(manager.get_channel("channel-0").is_some());

        // Transition to TryOpen state
        manager.try_open_channel("channel-0").unwrap();

        // Open the channel
        manager
            .open_channel("channel-0", "channel-1".to_string())
            .unwrap();

        // Send a packet
        let packet = manager
            .send_packet("channel-0", vec![1, 2, 3], Some(1000), None)
            .unwrap();

        assert_eq!(packet.sequence, 1);
        assert_eq!(manager.pending_packets.len(), 1);
    }
}
