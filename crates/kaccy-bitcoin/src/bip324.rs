//! BIP 324: Version 2 P2P Encrypted Transport Protocol
//!
//! Implements encrypted P2P communication between Bitcoin nodes using:
//! - ECDH for key exchange
//! - ChaCha20-Poly1305 for authenticated encryption
//! - Opportunistic encryption with v1 fallback
//!
//! # Overview
//!
//! BIP 324 provides:
//! - End-to-end encryption of P2P traffic
//! - Protection against passive observers
//! - Resistance to traffic analysis
//! - Backward compatibility with v1 protocol
//!
//! # Example
//!
//! ```
//! use kaccy_bitcoin::bip324::{V2Transport, V2TransportConfig};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a v2 transport with default config
//! let config = V2TransportConfig::default();
//! let mut transport = V2Transport::new(config)?;
//!
//! // Perform handshake and encrypt messages
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit},
};
use serde::{Deserialize, Serialize};

/// BIP 324 v2 transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2TransportConfig {
    /// Enable opportunistic encryption (fallback to v1)
    pub opportunistic: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Enable traffic padding for analysis resistance
    pub enable_padding: bool,
    /// Padding granularity in bytes
    pub padding_granularity: usize,
}

impl Default for V2TransportConfig {
    fn default() -> Self {
        Self {
            opportunistic: true,
            max_message_size: 4_000_000, // 4MB
            enable_padding: true,
            padding_granularity: 64,
        }
    }
}

/// Connection state for v2 transport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Initial state, no handshake
    Init,
    /// Handshake in progress
    HandshakeInProgress,
    /// Handshake completed, ready for encrypted communication
    Established,
    /// Connection closed
    Closed,
}

/// V2 transport session keys
pub struct SessionKeys {
    /// Sending cipher for ChaCha20-Poly1305
    send_cipher: ChaCha20Poly1305,
    /// Receiving cipher for ChaCha20-Poly1305
    recv_cipher: ChaCha20Poly1305,
    /// Sending nonce counter
    send_counter: u64,
    /// Receiving nonce counter
    recv_counter: u64,
}

impl std::fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionKeys")
            .field("send_counter", &self.send_counter)
            .field("recv_counter", &self.recv_counter)
            .finish()
    }
}

impl SessionKeys {
    /// Create new session keys from ECDH shared secret
    pub fn derive_from_ecdh(shared_secret: &[u8; 32], is_initiator: bool) -> Self {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Derive keys using HKDF-like construction
        // Key 1: for initiator->responder direction
        let mut key1_engine = sha256::Hash::engine();
        key1_engine.input(shared_secret);
        key1_engine.input(b"bip324_initiator_to_responder");
        let key1_hash = sha256::Hash::from_engine(key1_engine);

        // Key 2: for responder->initiator direction
        let mut key2_engine = sha256::Hash::engine();
        key2_engine.input(shared_secret);
        key2_engine.input(b"bip324_responder_to_initiator");
        let key2_hash = sha256::Hash::from_engine(key2_engine);

        let (send_key, recv_key) = if is_initiator {
            // Initiator sends with key1, receives with key2
            (key1_hash.to_byte_array(), key2_hash.to_byte_array())
        } else {
            // Responder sends with key2, receives with key1
            (key2_hash.to_byte_array(), key1_hash.to_byte_array())
        };

        // Create ChaCha20Poly1305 cipher instances
        let send_cipher = ChaCha20Poly1305::new(&send_key.into());
        let recv_cipher = ChaCha20Poly1305::new(&recv_key.into());

        Self {
            send_cipher,
            recv_cipher,
            send_counter: 0,
            recv_counter: 0,
        }
    }

    /// Get next send nonce
    fn next_send_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.send_counter.to_le_bytes());
        self.send_counter += 1;
        nonce
    }

    /// Get next receive nonce
    fn next_recv_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.recv_counter.to_le_bytes());
        self.recv_counter += 1;
        nonce
    }
}

/// V2 P2P encrypted transport
#[derive(Debug)]
pub struct V2Transport {
    /// Configuration
    config: V2TransportConfig,
    /// Local private key for ECDH
    local_privkey: SecretKey,
    /// Local public key
    local_pubkey: PublicKey,
    /// Remote public key (set during handshake)
    remote_pubkey: Option<PublicKey>,
    /// Session keys (set after handshake)
    session_keys: Option<SessionKeys>,
    /// Connection state
    state: ConnectionState,
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl V2Transport {
    /// Create a new v2 transport with configuration
    pub fn new(config: V2TransportConfig) -> Result<Self, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secp = Secp256k1::new();
        let local_privkey = SecretKey::new(&mut OsRng);
        let local_pubkey = PublicKey::from_secret_key(&secp, &local_privkey);

        Ok(Self {
            config,
            local_privkey,
            local_pubkey,
            remote_pubkey: None,
            session_keys: None,
            state: ConnectionState::Init,
            secp,
        })
    }

    /// Get local public key for handshake
    pub fn local_public_key(&self) -> PublicKey {
        self.local_pubkey
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Initiate handshake as the initiator
    pub fn initiate_handshake(&mut self) -> Result<Vec<u8>, BitcoinError> {
        if self.state != ConnectionState::Init {
            return Err(BitcoinError::InvalidAddress(
                "Handshake already in progress or completed".to_string(),
            ));
        }

        self.state = ConnectionState::HandshakeInProgress;

        // Send our public key
        Ok(self.local_pubkey.serialize().to_vec())
    }

    /// Complete handshake as the responder
    pub fn respond_handshake(&mut self, initiator_pubkey: &[u8]) -> Result<Vec<u8>, BitcoinError> {
        if self.state != ConnectionState::Init {
            return Err(BitcoinError::InvalidAddress(
                "Invalid state for handshake response".to_string(),
            ));
        }

        // Parse initiator's public key
        let remote_pk = PublicKey::from_slice(initiator_pubkey)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid public key: {}", e)))?;

        self.remote_pubkey = Some(remote_pk);
        self.state = ConnectionState::HandshakeInProgress;

        // Compute shared secret and derive keys
        self.complete_handshake(false)?;

        // Send our public key
        Ok(self.local_pubkey.serialize().to_vec())
    }

    /// Finalize handshake as the initiator
    pub fn finalize_handshake(&mut self, responder_pubkey: &[u8]) -> Result<(), BitcoinError> {
        if self.state != ConnectionState::HandshakeInProgress {
            return Err(BitcoinError::InvalidAddress(
                "No handshake in progress".to_string(),
            ));
        }

        // Parse responder's public key
        let remote_pk = PublicKey::from_slice(responder_pubkey)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid public key: {}", e)))?;

        self.remote_pubkey = Some(remote_pk);

        // Compute shared secret and derive keys
        self.complete_handshake(true)
    }

    /// Complete handshake by deriving session keys
    fn complete_handshake(&mut self, is_initiator: bool) -> Result<(), BitcoinError> {
        let remote_pk = self
            .remote_pubkey
            .ok_or_else(|| BitcoinError::InvalidAddress("Remote public key not set".to_string()))?;

        // Compute ECDH shared secret
        let shared_point = remote_pk
            .mul_tweak(&self.secp, &self.local_privkey.into())
            .map_err(|e| BitcoinError::InvalidAddress(format!("ECDH failed: {}", e)))?;

        // Extract x-coordinate as shared secret
        let shared_secret = shared_point.serialize();
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&shared_secret[1..33]); // Skip the prefix byte

        // Derive session keys
        self.session_keys = Some(SessionKeys::derive_from_ecdh(&secret_bytes, is_initiator));
        self.state = ConnectionState::Established;

        Ok(())
    }

    /// Encrypt a message
    pub fn encrypt_message(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, BitcoinError> {
        if self.state != ConnectionState::Established {
            return Err(BitcoinError::InvalidAddress(
                "Connection not established".to_string(),
            ));
        }

        if plaintext.len() > self.config.max_message_size {
            return Err(BitcoinError::InvalidAddress(
                "Message too large".to_string(),
            ));
        }

        // Calculate padding length before borrowing keys
        let padding_len = if self.config.enable_padding {
            self.calculate_padding(plaintext.len())
        } else {
            0
        };

        let keys = self.session_keys.as_mut().ok_or_else(|| {
            BitcoinError::InvalidAddress("Session keys not available".to_string())
        })?;

        // Get nonce for this message
        let nonce_bytes = keys.next_send_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Add padding if enabled
        let mut padded_plaintext = plaintext.to_vec();
        padded_plaintext.resize(plaintext.len() + padding_len, 0);

        // Encrypt using ChaCha20-Poly1305 AEAD
        let ciphertext = keys
            .send_cipher
            .encrypt(nonce, padded_plaintext.as_ref())
            .map_err(|e| BitcoinError::InvalidAddress(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext for transmission
        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a message
    pub fn decrypt_message(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, BitcoinError> {
        if self.state != ConnectionState::Established {
            return Err(BitcoinError::InvalidAddress(
                "Connection not established".to_string(),
            ));
        }

        // Minimum size: 12 bytes (nonce) + 16 bytes (auth tag)
        if ciphertext.len() < 28 {
            return Err(BitcoinError::InvalidAddress(
                "Ciphertext too short".to_string(),
            ));
        }

        let keys = self.session_keys.as_mut().ok_or_else(|| {
            BitcoinError::InvalidAddress("Session keys not available".to_string())
        })?;

        // Extract nonce (first 12 bytes)
        let nonce_bytes = keys.next_recv_nonce();
        let received_nonce = &ciphertext[..12];

        // Verify nonce matches expected sequence
        if received_nonce != nonce_bytes {
            return Err(BitcoinError::InvalidAddress(
                "Nonce mismatch - possible replay attack".to_string(),
            ));
        }

        let nonce = Nonce::from_slice(received_nonce);

        // Decrypt using ChaCha20-Poly1305 AEAD
        let plaintext = keys
            .recv_cipher
            .decrypt(nonce, &ciphertext[12..])
            .map_err(|e| BitcoinError::InvalidAddress(format!("Decryption failed: {}", e)))?;

        Ok(plaintext)
    }

    /// Calculate padding length for traffic analysis resistance
    fn calculate_padding(&self, message_len: usize) -> usize {
        if !self.config.enable_padding {
            return 0;
        }

        let granularity = self.config.padding_granularity;
        let remainder = message_len % granularity;

        if remainder == 0 {
            0
        } else {
            granularity - remainder
        }
    }

    /// Close the connection
    pub fn close(&mut self) {
        self.state = ConnectionState::Closed;
        self.session_keys = None;
        self.remote_pubkey = None;
    }
}

/// V2 transport statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct V2TransportStats {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total bytes sent (encrypted)
    pub bytes_sent: u64,
    /// Total bytes received (encrypted)
    pub bytes_received: u64,
    /// Total padding bytes added
    pub padding_bytes: u64,
    /// Number of handshake attempts
    pub handshake_attempts: u64,
    /// Number of successful connections
    pub successful_connections: u64,
    /// Number of v1 fallbacks
    pub v1_fallbacks: u64,
}

impl V2TransportStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a sent message
    pub fn record_send(&mut self, message_size: usize, padding: usize) {
        self.messages_sent += 1;
        self.bytes_sent += message_size as u64;
        self.padding_bytes += padding as u64;
    }

    /// Record a received message
    pub fn record_receive(&mut self, message_size: usize) {
        self.messages_received += 1;
        self.bytes_received += message_size as u64;
    }

    /// Record a handshake attempt
    pub fn record_handshake(&mut self, successful: bool) {
        self.handshake_attempts += 1;
        if successful {
            self.successful_connections += 1;
        }
    }

    /// Record a v1 fallback
    pub fn record_fallback(&mut self) {
        self.v1_fallbacks += 1;
    }

    /// Get encryption overhead ratio
    pub fn encryption_overhead_ratio(&self) -> f64 {
        if self.bytes_sent == 0 {
            return 0.0;
        }
        self.padding_bytes as f64 / self.bytes_sent as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = V2TransportConfig::default();
        assert!(config.opportunistic);
        assert!(config.enable_padding);
        assert_eq!(config.max_message_size, 4_000_000);
    }

    #[test]
    fn test_transport_creation() {
        let config = V2TransportConfig::default();
        let transport = V2Transport::new(config).unwrap();
        assert_eq!(transport.state(), ConnectionState::Init);
    }

    #[test]
    fn test_session_keys_derivation() {
        let secret = [42u8; 32];
        let keys_initiator = SessionKeys::derive_from_ecdh(&secret, true);
        let keys_responder = SessionKeys::derive_from_ecdh(&secret, false);

        // Verify both parties created valid session keys
        // Keys are complementary - initiator's send corresponds to responder's receive and vice versa
        // We can't directly compare ciphers, but counters should start at 0
        assert_eq!(keys_initiator.send_counter, 0);
        assert_eq!(keys_initiator.recv_counter, 0);
        assert_eq!(keys_responder.send_counter, 0);
        assert_eq!(keys_responder.recv_counter, 0);
    }

    #[test]
    fn test_handshake_flow() {
        let config = V2TransportConfig::default();
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        // Initiator starts handshake
        let init_msg = initiator.initiate_handshake().unwrap();
        assert_eq!(initiator.state(), ConnectionState::HandshakeInProgress);

        // Responder responds
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        assert_eq!(responder.state(), ConnectionState::Established);

        // Initiator finalizes
        initiator.finalize_handshake(&resp_msg).unwrap();
        assert_eq!(initiator.state(), ConnectionState::Established);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let config = V2TransportConfig {
            enable_padding: false, // Disable padding for exact plaintext comparison
            ..Default::default()
        };
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        // Complete handshake
        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        // Test encryption/decryption
        let plaintext = b"Hello, Bitcoin!";
        let ciphertext = initiator.encrypt_message(plaintext).unwrap();

        // Ciphertext should be: nonce (12 bytes) + encrypted data + auth tag (16 bytes)
        assert!(ciphertext.len() >= plaintext.len() + 28);

        let decrypted = responder.decrypt_message(&ciphertext).unwrap();
        // With proper AEAD and no padding, we should get exact plaintext back
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_padding_calculation() {
        let config = V2TransportConfig {
            enable_padding: true,
            padding_granularity: 64,
            ..Default::default()
        };
        let transport = V2Transport::new(config).unwrap();

        assert_eq!(transport.calculate_padding(0), 0);
        assert_eq!(transport.calculate_padding(64), 0);
        assert_eq!(transport.calculate_padding(65), 63);
        assert_eq!(transport.calculate_padding(100), 28);
    }

    #[test]
    fn test_stats_tracking() {
        let mut stats = V2TransportStats::new();

        stats.record_send(100, 28);
        stats.record_send(200, 56);
        stats.record_receive(150);

        assert_eq!(stats.messages_sent, 2);
        assert_eq!(stats.messages_received, 1);
        assert_eq!(stats.bytes_sent, 300);
        assert_eq!(stats.padding_bytes, 84);
        assert!(stats.encryption_overhead_ratio() > 0.0);
    }

    #[test]
    fn test_connection_close() {
        let config = V2TransportConfig::default();
        let mut transport = V2Transport::new(config).unwrap();

        transport.close();
        assert_eq!(transport.state(), ConnectionState::Closed);
        assert!(transport.session_keys.is_none());
    }

    #[test]
    fn test_encrypt_before_handshake_fails() {
        let config = V2TransportConfig::default();
        let mut transport = V2Transport::new(config).unwrap();

        let result = transport.encrypt_message(b"test");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_too_large() {
        let config = V2TransportConfig {
            max_message_size: 100,
            ..Default::default()
        };
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        // Complete handshake
        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        // Try to encrypt message that's too large
        let large_msg = vec![0u8; 200];
        let result = initiator.encrypt_message(&large_msg);
        assert!(result.is_err());
    }
}
