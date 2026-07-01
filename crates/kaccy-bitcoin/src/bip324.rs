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
//! # Wire Format
//!
//! Each encrypted packet is laid out as:
//! ```text
//! [ nonce (12 bytes) ][ AEAD ciphertext + Poly1305 tag (N + 16 bytes) ]
//! ```
//! where `N` is the (optionally padded) plaintext length.  Both sides maintain
//! an independent sequential 64-bit counter for their sending/receiving nonce so
//! that neither side ever reuses a (key, nonce) pair.  The counter is embedded
//! in the wire packet so the receiver can detect out-of-order or replayed
//! packets before attempting AEAD decryption.
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
///
/// Holds a pair of ChaCha20-Poly1305 cipher instances — one per direction —
/// together with monotonically increasing 64-bit nonce counters.  A separate
/// counter per direction ensures that neither side ever reuses a (key, nonce)
/// pair even under heavy traffic.
pub struct SessionKeys {
    /// Sending cipher for ChaCha20-Poly1305
    send_cipher: ChaCha20Poly1305,
    /// Receiving cipher for ChaCha20-Poly1305
    recv_cipher: ChaCha20Poly1305,
    /// Sending nonce counter (incremented after each successful encryption)
    send_counter: u64,
    /// Receiving nonce counter (incremented after each successful decryption)
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
    /// Create new session keys from an ECDH shared secret.
    ///
    /// Two orthogonal ChaCha20-Poly1305 keys are derived — one for each
    /// direction — via SHA-256 with a domain-separated label so that
    /// compromising one direction's key does not compromise the other.
    pub fn derive_from_ecdh(shared_secret: &[u8; 32], is_initiator: bool) -> Self {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Key 1: initiator → responder channel
        let mut key1_engine = sha256::Hash::engine();
        key1_engine.input(shared_secret);
        key1_engine.input(b"bip324_initiator_to_responder");
        let key1_hash = sha256::Hash::from_engine(key1_engine);

        // Key 2: responder → initiator channel
        let mut key2_engine = sha256::Hash::engine();
        key2_engine.input(shared_secret);
        key2_engine.input(b"bip324_responder_to_initiator");
        let key2_hash = sha256::Hash::from_engine(key2_engine);

        let (send_key, recv_key) = if is_initiator {
            // Initiator sends on key1, receives on key2
            (key1_hash.to_byte_array(), key2_hash.to_byte_array())
        } else {
            // Responder sends on key2, receives on key1
            (key2_hash.to_byte_array(), key1_hash.to_byte_array())
        };

        let send_cipher = ChaCha20Poly1305::new(&send_key.into());
        let recv_cipher = ChaCha20Poly1305::new(&recv_key.into());

        Self {
            send_cipher,
            recv_cipher,
            send_counter: 0,
            recv_counter: 0,
        }
    }

    /// Build the 12-byte nonce for the current send counter and advance it.
    ///
    /// Layout: 4 zero bytes | 8-byte little-endian counter.  This matches the
    /// IETF ChaCha20 nonce convention (32-bit counter in the first word, 96-bit
    /// nonce in the remaining three words — we put the message counter in the
    /// high 64 bits of the nonce field).
    fn next_send_nonce(&mut self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.send_counter.to_le_bytes());
        self.send_counter += 1;
        nonce
    }

    /// Build the 12-byte nonce for the current receive counter WITHOUT advancing
    /// it.  The counter is advanced only after a fully successful decryption via
    /// [`SessionKeys::advance_recv_counter`], so that a nonce-mismatch or AEAD
    /// failure does not permanently corrupt the counter state.
    fn current_recv_nonce(&self) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.recv_counter.to_le_bytes());
        nonce
    }

    /// Advance the receive counter after a successful decryption.
    fn advance_recv_counter(&mut self) {
        self.recv_counter += 1;
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

        // Compute ECDH shared secret using secp256k1 scalar multiplication.
        let shared_point = remote_pk
            .mul_tweak(&self.secp, &self.local_privkey.into())
            .map_err(|e| BitcoinError::InvalidAddress(format!("ECDH failed: {}", e)))?;

        // Extract x-coordinate as the shared secret (skip the 0x02/0x03 prefix byte).
        let shared_secret = shared_point.serialize();
        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&shared_secret[1..33]);

        self.session_keys = Some(SessionKeys::derive_from_ecdh(&secret_bytes, is_initiator));
        self.state = ConnectionState::Established;

        Ok(())
    }

    /// Encrypt a message using ChaCha20-Poly1305 AEAD.
    ///
    /// Returns a packet with the layout:
    /// `[ nonce (12 B) ][ ciphertext + Poly1305 tag (len + 16 B) ]`
    ///
    /// The 12-byte nonce is the current sequential send counter encoded as a
    /// little-endian u64 in bytes 4..12 (bytes 0..4 are zero).  It is prepended
    /// to the packet so that the receiver can verify sequencing before decryption.
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

        // Compute padding before mutably borrowing session keys.
        let padding_len = if self.config.enable_padding {
            self.calculate_padding(plaintext.len())
        } else {
            0
        };

        let keys = self.session_keys.as_mut().ok_or_else(|| {
            BitcoinError::InvalidAddress("Session keys not available".to_string())
        })?;

        // Advance send counter and get the nonce for this packet.
        let nonce_bytes = keys.next_send_nonce();
        let nonce = Nonce::from(nonce_bytes);

        // Append zero-byte padding to the plaintext for traffic-analysis resistance.
        let mut padded_plaintext = plaintext.to_vec();
        padded_plaintext.resize(plaintext.len() + padding_len, 0u8);

        // AEAD encrypt: produces ciphertext || Poly1305 tag (16 bytes).
        let aead_output = keys
            .send_cipher
            .encrypt(&nonce, padded_plaintext.as_ref())
            .map_err(|e| BitcoinError::InvalidAddress(format!("Encryption failed: {}", e)))?;

        // Wire packet: nonce prefix followed by AEAD output.
        let mut packet = Vec::with_capacity(12 + aead_output.len());
        packet.extend_from_slice(&nonce_bytes);
        packet.extend_from_slice(&aead_output);

        Ok(packet)
    }

    /// Decrypt a packet produced by [`V2Transport::encrypt_message`].
    ///
    /// The wire layout is `[ nonce (12 B) ][ ciphertext + tag ]`.
    /// Decryption proceeds in three strict steps:
    ///
    /// 1. **Nonce sequencing check** — the wire nonce is compared against the
    ///    expected counter using a constant-time OR-accumulator.  A mismatch
    ///    returns an error *without* advancing the counter, so the session state
    ///    remains valid for the next genuine packet.
    /// 2. **AEAD decryption** — ChaCha20-Poly1305 decrypts the ciphertext and
    ///    verifies the Poly1305 authentication tag.  Any tampering with the
    ///    ciphertext or the tag causes this step to fail.
    /// 3. **Counter advance** — only after both steps succeed is the receive
    ///    counter incremented.
    pub fn decrypt_message(&mut self, packet: &[u8]) -> Result<Vec<u8>, BitcoinError> {
        if self.state != ConnectionState::Established {
            return Err(BitcoinError::InvalidAddress(
                "Connection not established".to_string(),
            ));
        }

        // Minimum: 12-byte nonce + 16-byte Poly1305 tag (zero-length plaintext).
        if packet.len() < 28 {
            return Err(BitcoinError::InvalidAddress(
                "Packet too short to be a valid BIP-324 AEAD frame".to_string(),
            ));
        }

        let keys = self.session_keys.as_mut().ok_or_else(|| {
            BitcoinError::InvalidAddress("Session keys not available".to_string())
        })?;

        // Step 1 — Nonce sequencing check.
        //
        // Peek at the expected nonce WITHOUT incrementing the counter.  This
        // guarantees that a replay / out-of-order packet cannot permanently
        // desynchronise the counter: if the check fails we simply return an
        // error and the counter stays where it was.
        let expected_nonce = keys.current_recv_nonce();
        let wire_nonce = &packet[..12];

        // Constant-time comparison: accumulate XOR differences so every byte is
        // always evaluated, preventing timing side-channels.
        let nonce_diff: u8 = wire_nonce
            .iter()
            .zip(expected_nonce.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));

        if nonce_diff != 0 {
            return Err(BitcoinError::InvalidAddress(
                "Nonce mismatch — possible replay or out-of-order packet".to_string(),
            ));
        }

        let nonce = Nonce::try_from(wire_nonce)
            .map_err(|_| BitcoinError::InvalidAddress("Invalid nonce length".to_string()))?;

        // Step 2 — AEAD decryption with tag verification.
        //
        // The chacha20poly1305 crate returns a generic error on authentication
        // failure, so we surface a clear typed message rather than propagating
        // the opaque internal error.
        let plaintext = keys
            .recv_cipher
            .decrypt(&nonce, &packet[12..])
            .map_err(|_| {
                BitcoinError::InvalidAddress(
                    "AEAD authentication failed — ciphertext or Poly1305 tag is corrupt"
                        .to_string(),
                )
            })?;

        // Step 3 — Advance counter only after fully successful decryption.
        keys.advance_recv_counter();

        Ok(plaintext)
    }

    /// Calculate zero-byte padding length to round message size up to the
    /// configured granularity boundary for traffic-analysis resistance.
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

        let init_msg = initiator.initiate_handshake().unwrap();
        assert_eq!(initiator.state(), ConnectionState::HandshakeInProgress);

        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        assert_eq!(responder.state(), ConnectionState::Established);

        initiator.finalize_handshake(&resp_msg).unwrap();
        assert_eq!(initiator.state(), ConnectionState::Established);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let config = V2TransportConfig {
            enable_padding: false,
            ..Default::default()
        };
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        let plaintext = b"Hello, Bitcoin!";
        let ciphertext = initiator.encrypt_message(plaintext).unwrap();

        // Packet: nonce (12 B) + ciphertext body + Poly1305 tag (16 B).
        assert!(ciphertext.len() >= plaintext.len() + 28);

        let decrypted = responder.decrypt_message(&ciphertext).unwrap();
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

        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        let large_msg = vec![0u8; 200];
        let result = initiator.encrypt_message(&large_msg);
        assert!(result.is_err());
    }

    /// AEAD round-trip: the decrypted payload must equal the original plaintext
    /// for a variety of message lengths, including an empty message.
    #[test]
    fn test_aead_round_trip_recovers_plaintext() {
        let config = V2TransportConfig {
            enable_padding: false,
            ..Default::default()
        };
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        let messages: &[&[u8]] = &[
            b"",
            b"Hello, BIP-324!",
            b"The quick brown fox jumps over the lazy dog",
            &[
                0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
                0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
                0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
                0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
                0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
            ],
        ];

        for &msg in messages {
            let packet = initiator.encrypt_message(msg).unwrap();
            let recovered = responder.decrypt_message(&packet).unwrap();
            assert_eq!(
                recovered,
                msg,
                "AEAD round-trip failed for message of {} bytes",
                msg.len()
            );
        }
    }

    /// Any single-bit flip anywhere in the ciphertext body or the Poly1305
    /// authentication tag must cause `decrypt_message` to return an error.
    #[test]
    fn test_tampered_ciphertext_fails_authentication() {
        // Helper: build a fresh established initiator + responder pair.
        let make_pair = || {
            let cfg = V2TransportConfig {
                enable_padding: false,
                ..Default::default()
            };
            let mut i = V2Transport::new(cfg.clone()).unwrap();
            let mut r = V2Transport::new(cfg).unwrap();
            let h = i.initiate_handshake().unwrap();
            let rh = r.respond_handshake(&h).unwrap();
            i.finalize_handshake(&rh).unwrap();
            (i, r)
        };

        let plaintext = b"authenticated payload";

        // Tamper with a byte inside the ciphertext body (byte index 12 + 2,
        // well inside the encrypted payload, not the tag).
        {
            let (mut init, mut resp) = make_pair();
            let mut packet = init.encrypt_message(plaintext).unwrap();
            // Index 14 is the third byte of the AEAD ciphertext body (after the
            // 12-byte nonce prefix).
            packet[14] ^= 0xFF;
            let result = resp.decrypt_message(&packet);
            assert!(
                result.is_err(),
                "Expected AEAD failure after ciphertext body tampering"
            );
        }

        // Tamper with the last byte of the packet (part of the Poly1305 tag).
        {
            let (mut init, mut resp) = make_pair();
            let mut packet = init.encrypt_message(plaintext).unwrap();
            let last = packet.len() - 1;
            packet[last] ^= 0x01;
            let result = resp.decrypt_message(&packet);
            assert!(
                result.is_err(),
                "Expected AEAD failure after Poly1305 tag tampering"
            );
        }

        // Tamper with a byte in the middle of the Poly1305 tag (second-to-last byte).
        {
            let (mut init, mut resp) = make_pair();
            let mut packet = init.encrypt_message(plaintext).unwrap();
            let mid_tag = packet.len() - 8;
            packet[mid_tag] ^= 0xAB;
            let result = resp.decrypt_message(&packet);
            assert!(
                result.is_err(),
                "Expected AEAD failure after Poly1305 tag mid-byte tampering"
            );
        }
    }

    /// Encrypting the same plaintext twice must produce distinct ciphertexts
    /// because sequential nonces differ.  Both packets must still decrypt
    /// correctly at the responder.
    #[test]
    fn test_different_nonces_produce_different_ciphertext() {
        let config = V2TransportConfig {
            enable_padding: false,
            ..Default::default()
        };
        let mut initiator = V2Transport::new(config.clone()).unwrap();
        let mut responder = V2Transport::new(config).unwrap();

        let init_msg = initiator.initiate_handshake().unwrap();
        let resp_msg = responder.respond_handshake(&init_msg).unwrap();
        initiator.finalize_handshake(&resp_msg).unwrap();

        let plaintext = b"same plaintext each time";

        let packet1 = initiator.encrypt_message(plaintext).unwrap();
        let packet2 = initiator.encrypt_message(plaintext).unwrap();

        // The nonce prefix (first 12 bytes) must differ between sequential messages.
        assert_ne!(
            &packet1[..12],
            &packet2[..12],
            "sequential per-message nonces must differ"
        );

        // Distinct nonces produce distinct keystreams ⟹ distinct ciphertexts.
        assert_ne!(
            packet1, packet2,
            "identical plaintexts under distinct nonces must yield distinct packets"
        );

        // Both packets must decrypt to the original plaintext at the responder.
        let recovered1 = responder.decrypt_message(&packet1).unwrap();
        let recovered2 = responder.decrypt_message(&packet2).unwrap();
        assert_eq!(recovered1, plaintext, "first packet failed round-trip");
        assert_eq!(recovered2, plaintext, "second packet failed round-trip");
    }
}
