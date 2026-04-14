//! BIP 340 Schnorr signature wrapper for convenient key generation, signing, and verification.
//!
//! This module provides a high-level interface over the `secp256k1` crate's Schnorr
//! implementation, using x-only public keys as specified in BIP 340 and Taproot (BIP 341).
//!
//! All secp256k1 types come from `bitcoin::secp256k1` (secp256k1 v0.29 as re-exported by the
//! `bitcoin` crate) to ensure type compatibility with the rest of the kaccy-bitcoin codebase.
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::schnorr::{SchnorrKeyPair, SchnorrSigner, SchnorrUtils};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate a fresh key pair
//! let kp = SchnorrKeyPair::generate()?;
//! let pubkey = kp.public_key();
//! assert_eq!(pubkey.len(), 32);
//!
//! // Produce a message digest for signing
//! let msg = SchnorrUtils::message_from_bytes(b"Hello, Taproot!");
//! assert_eq!(msg.len(), 32);
//! # Ok(())
//! # }
//! ```

use bitcoin::hashes::{Hash, sha256};
use bitcoin::secp256k1::rand::thread_rng;
use bitcoin::secp256k1::{Keypair, Message, Secp256k1, XOnlyPublicKey, schnorr::Signature};
use thiserror::Error;

/// Errors arising from Schnorr key operations or signature verification.
#[derive(Debug, Error)]
pub enum SchnorrError {
    /// The provided secret key bytes are invalid.
    #[error("Invalid secret key: {0}")]
    InvalidSecretKey(String),

    /// The provided public key bytes are invalid.
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    /// The provided signature bytes are invalid.
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    /// The signing operation failed.
    #[error("Signing failed: {0}")]
    SigningFailed(String),

    /// Signature verification failed (signature does not match).
    #[error("Signature verification failed")]
    VerificationFailed,

    /// The provided message is not valid (should be 32 bytes).
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// One item in batch verification failed.
    ///
    /// Contains the zero-based index of the first failing item.
    #[error("Batch verification failed at index {0}")]
    BatchVerificationFailed(usize),
}

/// A BIP 340 Schnorr key pair (x-only public key + secret key).
///
/// Stores both the secp256k1 `Keypair` and the raw secret key bytes for easy access.
#[derive(Debug)]
pub struct SchnorrKeyPair {
    /// Underlying secp256k1 keypair
    keypair: Keypair,
    /// Raw secret key bytes (32 bytes), stored for signing convenience
    secret_bytes: [u8; 32],
}

impl SchnorrKeyPair {
    /// Generate a new random Schnorr key pair using the OS thread RNG.
    pub fn generate() -> Result<Self, SchnorrError> {
        let secp = Secp256k1::new();
        let keypair = Keypair::new(&secp, &mut thread_rng());
        let secret_bytes = keypair.secret_key().secret_bytes();
        Ok(Self {
            keypair,
            secret_bytes,
        })
    }

    /// Construct a `SchnorrKeyPair` from raw secret key bytes (32 bytes).
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self, SchnorrError> {
        let secp = Secp256k1::new();
        let secret_key = bitcoin::secp256k1::SecretKey::from_slice(bytes).map_err(|e| {
            SchnorrError::InvalidSecretKey(format!("invalid secret key bytes: {}", e))
        })?;
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let secret_bytes = keypair.secret_key().secret_bytes();
        Ok(Self {
            keypair,
            secret_bytes,
        })
    }

    /// Return the x-only public key as a 32-byte array (BIP 340 format).
    pub fn public_key(&self) -> [u8; 32] {
        let (xonly, _parity) = XOnlyPublicKey::from_keypair(&self.keypair);
        xonly.serialize()
    }

    /// Return the x-only public key as a `Vec<u8>`.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key().to_vec()
    }

    /// Return the raw secret key bytes (32 bytes).
    ///
    /// These bytes can be passed to [`SchnorrSigner::sign`].
    pub fn secret_key_bytes(&self) -> &[u8; 32] {
        &self.secret_bytes
    }
}

/// Stateless Schnorr signing and verification utilities (BIP 340).
pub struct SchnorrSigner;

impl SchnorrSigner {
    /// Sign a 32-byte message hash with the given secret key bytes.
    ///
    /// Uses a random nonce via `sign_schnorr`.
    /// Returns the 64-byte Schnorr signature.
    pub fn sign(secret_bytes: &[u8], message: &[u8; 32]) -> Result<[u8; 64], SchnorrError> {
        let secp = Secp256k1::new();
        let secret_key = bitcoin::secp256k1::SecretKey::from_slice(secret_bytes)
            .map_err(|e| SchnorrError::InvalidSecretKey(format!("{}", e)))?;
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let msg = Message::from_digest(*message);
        let sig = secp.sign_schnorr(&msg, &keypair);
        let mut out = [0u8; 64];
        out.copy_from_slice(sig.as_ref());
        Ok(out)
    }

    /// Sign a 32-byte message hash using a deterministic auxiliary randomness value.
    ///
    /// The `aux_rand` parameter provides 32 bytes of additional entropy mixed into
    /// the nonce generation (see BIP 340 §Default Signing).
    pub fn sign_with_aux_rand(
        secret_bytes: &[u8],
        message: &[u8; 32],
        aux_rand: &[u8; 32],
    ) -> Result<[u8; 64], SchnorrError> {
        let secp = Secp256k1::new();
        let secret_key = bitcoin::secp256k1::SecretKey::from_slice(secret_bytes)
            .map_err(|e| SchnorrError::InvalidSecretKey(format!("{}", e)))?;
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let msg = Message::from_digest(*message);
        let sig = secp.sign_schnorr_with_aux_rand(&msg, &keypair, aux_rand);
        let mut out = [0u8; 64];
        out.copy_from_slice(sig.as_ref());
        Ok(out)
    }

    /// Verify a 64-byte Schnorr signature against a 32-byte public key and 32-byte message.
    ///
    /// Returns `Ok(())` on success, [`SchnorrError::VerificationFailed`] on failure.
    pub fn verify(
        public_key: &[u8; 32],
        message: &[u8; 32],
        signature: &[u8; 64],
    ) -> Result<(), SchnorrError> {
        let secp = Secp256k1::verification_only();
        let xonly = XOnlyPublicKey::from_slice(public_key.as_ref())
            .map_err(|e| SchnorrError::InvalidPublicKey(format!("{}", e)))?;
        let sig = Signature::from_slice(signature.as_ref())
            .map_err(|e| SchnorrError::InvalidSignature(format!("{}", e)))?;
        let msg = Message::from_digest(*message);
        secp.verify_schnorr(&sig, &msg, &xonly)
            .map_err(|_| SchnorrError::VerificationFailed)
    }

    /// Verify multiple Schnorr signatures in a batch.
    ///
    /// Each item is `(public_key_32, message_32, signature_64)`.
    /// Verification is performed individually (secp256k1 does not expose a native
    /// batch-verify API). Returns the index of the first failing item.
    pub fn batch_verify(items: &[([u8; 32], [u8; 32], [u8; 64])]) -> Result<(), SchnorrError> {
        for (idx, (pubkey, message, signature)) in items.iter().enumerate() {
            Self::verify(pubkey, message, signature)
                .map_err(|_| SchnorrError::BatchVerificationFailed(idx))?;
        }
        Ok(())
    }
}

/// Utility functions for Schnorr message preparation.
pub struct SchnorrUtils;

impl SchnorrUtils {
    /// Hash arbitrary bytes with SHA-256 to produce a 32-byte message digest.
    ///
    /// This is a common pattern for signing structured data without requiring
    /// the caller to perform their own hashing.
    pub fn message_from_bytes(data: &[u8]) -> [u8; 32] {
        sha256::Hash::hash(data).to_byte_array()
    }

    /// Validate that a byte slice is exactly 64 bytes and return it as a fixed array.
    ///
    /// Returns [`SchnorrError::InvalidSignature`] if the length is wrong.
    pub fn normalize_signature(sig: &[u8]) -> Result<[u8; 64], SchnorrError> {
        if sig.len() != 64 {
            return Err(SchnorrError::InvalidSignature(format!(
                "expected 64 bytes, got {}",
                sig.len()
            )));
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(sig);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let kp = SchnorrKeyPair::generate().expect("generate keypair");
        let pk = kp.public_key();
        assert_eq!(pk.len(), 32);
        // Public key must not be all zeros
        assert_ne!(pk, [0u8; 32]);
    }

    #[test]
    fn test_sign_verify_roundtrip() {
        let kp = SchnorrKeyPair::generate().expect("generate keypair");
        let msg = SchnorrUtils::message_from_bytes(b"test message for signing");
        let sig = SchnorrSigner::sign(kp.secret_key_bytes(), &msg).expect("sign");
        let pk = kp.public_key();
        SchnorrSigner::verify(&pk, &msg, &sig).expect("verify");
    }

    #[test]
    fn test_verify_invalid_signature_fails() {
        let kp = SchnorrKeyPair::generate().expect("generate keypair");
        let msg = SchnorrUtils::message_from_bytes(b"test message");
        // Use a signature full of zeros — not a valid Schnorr sig
        let bad_sig = [0u8; 64];
        let pk = kp.public_key();
        let result = SchnorrSigner::verify(&pk, &msg, &bad_sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wrong_pubkey_fails() {
        let kp1 = SchnorrKeyPair::generate().expect("generate keypair 1");
        let kp2 = SchnorrKeyPair::generate().expect("generate keypair 2");
        let msg = SchnorrUtils::message_from_bytes(b"test message");
        let sig = SchnorrSigner::sign(kp1.secret_key_bytes(), &msg).expect("sign");
        // Verify with a different public key — should fail
        let wrong_pk = kp2.public_key();
        let result = SchnorrSigner::verify(&wrong_pk, &msg, &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_verify_all_valid() {
        let mut items: Vec<([u8; 32], [u8; 32], [u8; 64])> = Vec::new();
        for i in 0..5u8 {
            let kp = SchnorrKeyPair::generate().expect("generate");
            let msg = SchnorrUtils::message_from_bytes(&[i, i, i]);
            let sig = SchnorrSigner::sign(kp.secret_key_bytes(), &msg).expect("sign");
            items.push((kp.public_key(), msg, sig));
        }
        SchnorrSigner::batch_verify(&items).expect("batch verify all valid");
    }

    #[test]
    fn test_batch_verify_one_invalid() {
        let mut items: Vec<([u8; 32], [u8; 32], [u8; 64])> = Vec::new();
        for i in 0..3u8 {
            let kp = SchnorrKeyPair::generate().expect("generate");
            let msg = SchnorrUtils::message_from_bytes(&[i, i, i]);
            let sig = SchnorrSigner::sign(kp.secret_key_bytes(), &msg).expect("sign");
            items.push((kp.public_key(), msg, sig));
        }
        // Corrupt the signature at index 1
        items[1].2 = [0xffu8; 64];
        let result = SchnorrSigner::batch_verify(&items);
        assert!(matches!(
            result,
            Err(SchnorrError::BatchVerificationFailed(1))
        ));
    }

    #[test]
    fn test_message_from_bytes() {
        let msg = SchnorrUtils::message_from_bytes(b"hello bitcoin");
        assert_eq!(msg.len(), 32);
        // Same input must produce the same hash
        let msg2 = SchnorrUtils::message_from_bytes(b"hello bitcoin");
        assert_eq!(msg, msg2);
        // Different input must produce different hash
        let msg3 = SchnorrUtils::message_from_bytes(b"hello taproot");
        assert_ne!(msg, msg3);
    }

    #[test]
    fn test_keypair_from_secret_bytes() {
        let kp = SchnorrKeyPair::generate().expect("generate");
        let secret = kp.secret_key_bytes().to_vec();
        let kp2 = SchnorrKeyPair::from_secret_bytes(&secret).expect("from_secret_bytes");
        // Same secret key → same public key
        assert_eq!(kp.public_key(), kp2.public_key());
    }

    #[test]
    fn test_normalize_signature_valid() {
        let bytes = vec![0xabu8; 64];
        let out = SchnorrUtils::normalize_signature(&bytes).expect("normalize");
        assert_eq!(out.len(), 64);
    }

    #[test]
    fn test_normalize_signature_invalid_length() {
        let bytes = vec![0u8; 32]; // too short
        let result = SchnorrUtils::normalize_signature(&bytes);
        assert!(matches!(result, Err(SchnorrError::InvalidSignature(_))));
    }

    #[test]
    fn test_sign_with_aux_rand() {
        let kp = SchnorrKeyPair::generate().expect("generate");
        let msg = SchnorrUtils::message_from_bytes(b"aux rand test");
        let aux = [0x42u8; 32];
        let sig = SchnorrSigner::sign_with_aux_rand(kp.secret_key_bytes(), &msg, &aux)
            .expect("sign with aux rand");
        SchnorrSigner::verify(&kp.public_key(), &msg, &sig).expect("verify");
    }
}
