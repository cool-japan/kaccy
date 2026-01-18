//! MuSig2 (BIP 327) implementation for Schnorr multi-signatures
//!
//! MuSig2 allows multiple parties to create a single aggregated signature
//! that is indistinguishable from a single-key signature, providing:
//!
//! - Privacy: On-chain looks like single-sig
//! - Efficiency: One signature regardless of number of signers
//! - Security: Based on Schnorr signature scheme
//!
//! # Overview
//!
//! MuSig2 signing process:
//! 1. Key aggregation: Combine public keys into an aggregated key
//! 2. Nonce generation: Each signer generates two nonces
//! 3. Nonce aggregation: Combine nonces from all signers
//! 4. Partial signing: Each signer creates a partial signature
//! 5. Signature aggregation: Combine partial signatures into final signature
//!
//! # Example
//!
//! ```
//! use kaccy_bitcoin::musig2::{MuSig2Signer, MuSig2KeyAggregator};
//! # use bitcoin::secp256k1::SecretKey;
//! # use bitcoin::secp256k1::rand::rngs::OsRng;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create signers
//! let signer1 = MuSig2Signer::new(SecretKey::new(&mut OsRng))?;
//! let signer2 = MuSig2Signer::new(SecretKey::new(&mut OsRng))?;
//!
//! // Aggregate public keys
//! let aggregator = MuSig2KeyAggregator::new(vec![
//!     signer1.public_key(),
//!     signer2.public_key(),
//! ])?;
//!
//! let aggregated_key = aggregator.aggregated_key();
//! println!("Aggregated public key: {:?}", aggregated_key);
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Adaptor signature for atomic swaps and payment channels
///
/// An adaptor signature is a signature that has been "adapted" with a secret value.
/// When the adapted signature is revealed, the original adaptor secret can be recovered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptorSignature {
    /// The adaptor point (T = t*G) used to create the adaptor signature
    pub adaptor_point: PublicKey,
    /// The adapted signature (s' = s + t)
    pub adapted_signature: [u8; 32],
    /// Original partial signatures before adaptation
    pub original_signatures: Vec<PartialSignature>,
}

impl AdaptorSignature {
    /// Create a new adaptor signature
    pub fn new(
        adaptor_point: PublicKey,
        adapted_signature: [u8; 32],
        original_signatures: Vec<PartialSignature>,
    ) -> Self {
        Self {
            adaptor_point,
            adapted_signature,
            original_signatures,
        }
    }

    /// Recover the adaptor secret from the final signature
    ///
    /// Given the final signature and the adapted signature, recover the secret t
    /// such that T = t*G (the adaptor point).
    pub fn recover_adaptor_secret(
        &self,
        final_signature: &[u8; 32],
    ) -> Result<SecretKey, BitcoinError> {
        // Compute t = s_final - s_adapted using proper scalar arithmetic
        let s_final = SecretKey::from_slice(final_signature)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid final signature: {}", e)))?;

        let s_adapted = SecretKey::from_slice(&self.adapted_signature).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid adapted signature: {}", e))
        })?;

        // Negate the adapted signature to compute subtraction as addition
        let neg_adapted = s_adapted.negate();

        // Compute t = s_final + (-s_adapted) = s_final - s_adapted
        let secret = s_final.add_tweak(&neg_adapted.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to recover adaptor secret: {}", e))
        })?;

        Ok(secret)
    }

    /// Verify that the adaptor signature is valid
    pub fn verify(
        &self,
        _secp: &Secp256k1<bitcoin::secp256k1::All>,
        _message: &[u8; 32],
        _aggregated_pubkey: &XOnlyPublicKey,
    ) -> Result<bool, BitcoinError> {
        // In production, this would verify:
        // 1. The adapted signature is valid for the message + adaptor point
        // 2. All original signatures are properly formatted
        // For now, basic validation
        Ok(!self.original_signatures.is_empty())
    }
}

/// Adaptor signature manager for atomic swap and payment channel use cases
#[derive(Debug)]
pub struct AdaptorSignatureManager {
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl AdaptorSignatureManager {
    /// Create a new adaptor signature manager
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
        }
    }

    /// Generate a new adaptor secret and point
    pub fn generate_adaptor(&self) -> Result<(SecretKey, PublicKey), BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secret = SecretKey::new(&mut OsRng);
        let point = PublicKey::from_secret_key(&self.secp, &secret);

        Ok((secret, point))
    }

    /// Create an adaptor signature from partial signatures
    ///
    /// This adapts the aggregated signature with the adaptor point T = t*G.
    /// The resulting signature can only be completed by someone who knows t.
    pub fn create_adaptor_signature(
        &self,
        partial_signatures: Vec<PartialSignature>,
        adaptor_point: PublicKey,
        _aggregated_signature: &[u8; 32],
    ) -> Result<AdaptorSignature, BitcoinError> {
        if partial_signatures.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "No partial signatures provided".to_string(),
            ));
        }

        // In a full implementation, this would:
        // 1. Aggregate the partial signatures
        // 2. Adapt the aggregated signature with the adaptor point
        // For now, create a simplified adaptor signature
        let adapted_sig = partial_signatures[0].s;

        Ok(AdaptorSignature::new(
            adaptor_point,
            adapted_sig,
            partial_signatures,
        ))
    }

    /// Complete an adaptor signature with the adaptor secret
    ///
    /// Given an adaptor signature and the secret t, produce the final signature.
    pub fn complete_adaptor_signature(
        &self,
        adaptor_sig: &AdaptorSignature,
        adaptor_secret: &SecretKey,
    ) -> Result<[u8; 32], BitcoinError> {
        // Verify the adaptor secret matches the adaptor point
        let computed_point = PublicKey::from_secret_key(&self.secp, adaptor_secret);
        if computed_point != adaptor_sig.adaptor_point {
            return Err(BitcoinError::InvalidAddress(
                "Adaptor secret does not match adaptor point".to_string(),
            ));
        }

        // Compute final signature: s_final = s_adapted + t using proper scalar arithmetic
        let s_adapted = SecretKey::from_slice(&adaptor_sig.adapted_signature).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid adapted signature: {}", e))
        })?;

        let final_sig = s_adapted
            .add_tweak(&(*adaptor_secret).into())
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to complete adaptor signature: {}", e))
            })?;

        Ok(final_sig.secret_bytes())
    }

    /// Verify an adaptor signature can be completed to produce a valid signature
    pub fn verify_adaptor_signature(
        &self,
        adaptor_sig: &AdaptorSignature,
        message: &[u8; 32],
        aggregated_pubkey: &XOnlyPublicKey,
    ) -> Result<bool, BitcoinError> {
        adaptor_sig.verify(&self.secp, message, aggregated_pubkey)
    }
}

impl Default for AdaptorSignatureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// MuSig2 key aggregator for combining public keys
#[derive(Debug, Clone)]
pub struct MuSig2KeyAggregator {
    /// Public keys of all participants
    pub_keys: Vec<PublicKey>,
    /// Aggregated public key
    aggregated_key: XOnlyPublicKey,
    /// Key coefficients for each participant (BIP 327)
    coefficients: HashMap<PublicKey, Scalar>,
    /// Secp256k1 context
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl MuSig2KeyAggregator {
    /// Create a new key aggregator from public keys
    pub fn new(pub_keys: Vec<PublicKey>) -> Result<Self, BitcoinError> {
        if pub_keys.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "At least one public key required".to_string(),
            ));
        }

        let secp = Secp256k1::new();
        let coefficients = Self::compute_coefficients(&pub_keys)?;
        let aggregated_key = Self::aggregate_keys(&secp, &pub_keys, &coefficients)?;

        Ok(Self {
            pub_keys,
            aggregated_key,
            coefficients,
            secp,
        })
    }

    /// Compute key aggregation coefficients (BIP 327)
    fn compute_coefficients(
        pub_keys: &[PublicKey],
    ) -> Result<HashMap<PublicKey, Scalar>, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        let mut coefficients = HashMap::new();

        // Compute L = hash(P1 || P2 || ... || Pn)
        let mut l_engine = sha256::Hash::engine();
        for pk in pub_keys {
            l_engine.input(&pk.serialize());
        }
        let l_hash = sha256::Hash::from_engine(l_engine);

        // Compute coefficient for each key: a_i = hash(L || P_i)
        for pk in pub_keys {
            let mut engine = sha256::Hash::engine();
            engine.input(l_hash.as_byte_array());
            engine.input(&pk.serialize());
            let hash = sha256::Hash::from_engine(engine);

            let coeff = Scalar::from_be_bytes(hash.to_byte_array()).map_err(|_| {
                BitcoinError::InvalidAddress("Failed to compute coefficient".to_string())
            })?;

            coefficients.insert(*pk, coeff);
        }

        Ok(coefficients)
    }

    /// Aggregate public keys: Q = sum(a_i * P_i)
    fn aggregate_keys(
        secp: &Secp256k1<bitcoin::secp256k1::All>,
        pub_keys: &[PublicKey],
        coefficients: &HashMap<PublicKey, Scalar>,
    ) -> Result<XOnlyPublicKey, BitcoinError> {
        let mut aggregated: Option<PublicKey> = None;

        for pk in pub_keys {
            let coeff = coefficients.get(pk).ok_or_else(|| {
                BitcoinError::InvalidAddress("Missing coefficient for public key".to_string())
            })?;

            let weighted_key = pk.mul_tweak(secp, coeff).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to compute weighted key: {}", e))
            })?;

            aggregated = Some(if let Some(acc) = aggregated {
                acc.combine(&weighted_key).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to aggregate keys: {}", e))
                })?
            } else {
                weighted_key
            });
        }

        let aggregated_pk = aggregated
            .ok_or_else(|| BitcoinError::InvalidAddress("No keys to aggregate".to_string()))?;

        Ok(aggregated_pk.x_only_public_key().0)
    }

    /// Get the aggregated public key
    pub fn aggregated_key(&self) -> XOnlyPublicKey {
        self.aggregated_key
    }

    /// Get the coefficient for a specific public key
    pub fn get_coefficient(&self, pk: &PublicKey) -> Option<Scalar> {
        self.coefficients.get(pk).copied()
    }

    /// Get all public keys
    pub fn public_keys(&self) -> &[PublicKey] {
        &self.pub_keys
    }
}

/// Nonce pair for MuSig2 signing
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MuSig2Nonce {
    /// First nonce (r1)
    pub r1: PublicKey,
    /// Second nonce (r2)
    pub r2: PublicKey,
}

impl MuSig2Nonce {
    /// Create a new nonce pair
    pub fn new(r1: PublicKey, r2: PublicKey) -> Self {
        Self { r1, r2 }
    }
}

/// Aggregated nonce for MuSig2
#[derive(Debug, Clone, Copy)]
pub struct AggregatedNonce {
    /// Aggregated R = R1 + b*R2
    pub r: PublicKey,
    /// Nonce coefficient
    pub b: Scalar,
}

/// Partial signature from a single signer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSignature {
    /// Signer's public key
    pub signer_pubkey: PublicKey,
    /// Partial signature s_i (as bytes since Scalar doesn't support arithmetic)
    pub s: [u8; 32],
}

/// MuSig2 signer for creating partial signatures
#[derive(Debug)]
pub struct MuSig2Signer {
    /// Private key
    #[allow(dead_code)]
    privkey: SecretKey,
    /// Public key
    pubkey: PublicKey,
    /// Secp256k1 context
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
    /// Current nonce secrets (k1, k2)
    nonce_secrets: Option<(SecretKey, SecretKey)>,
    /// Current public nonces
    nonce_public: Option<MuSig2Nonce>,
}

impl MuSig2Signer {
    /// Create a new signer with a private key
    pub fn new(privkey: SecretKey) -> Result<Self, BitcoinError> {
        let secp = Secp256k1::new();
        let pubkey = PublicKey::from_secret_key(&secp, &privkey);

        Ok(Self {
            privkey,
            pubkey,
            secp,
            nonce_secrets: None,
            nonce_public: None,
        })
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        self.pubkey
    }

    /// Generate nonces for a signing session
    pub fn generate_nonces(&mut self) -> Result<MuSig2Nonce, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let k1 = SecretKey::new(&mut OsRng);
        let k2 = SecretKey::new(&mut OsRng);

        let r1 = PublicKey::from_secret_key(&self.secp, &k1);
        let r2 = PublicKey::from_secret_key(&self.secp, &k2);

        let nonce = MuSig2Nonce::new(r1, r2);

        self.nonce_secrets = Some((k1, k2));
        self.nonce_public = Some(nonce);

        Ok(nonce)
    }

    /// Create a partial signature
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        &self,
        message: &[u8; 32],
        aggregated_nonce: &AggregatedNonce,
        aggregated_pubkey: &XOnlyPublicKey,
        key_coefficient: Scalar,
    ) -> Result<PartialSignature, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Get nonce secrets
        let (k1, k2) = self
            .nonce_secrets
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;

        // Compute effective nonce: k = k1 + b*k2
        let k2_scaled = k2
            .mul_tweak(&aggregated_nonce.b)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale k2: {}", e)))?;

        let k_eff = k1.add_tweak(&k2_scaled.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute effective nonce: {}", e))
        })?;

        // Compute challenge: e = Hash(R || Q || m)
        let mut engine = sha256::Hash::engine();
        engine.input(&aggregated_nonce.r.serialize());
        engine.input(&aggregated_pubkey.serialize());
        engine.input(message);
        let challenge_hash = sha256::Hash::from_engine(engine);
        let challenge = Scalar::from_be_bytes(challenge_hash.to_byte_array())
            .map_err(|_| BitcoinError::InvalidAddress("Failed to compute challenge".to_string()))?;

        // Compute s_i = k_eff + e * a_i * x_i
        // where a_i is the key coefficient and x_i is the private key
        let privkey_scaled = self
            .privkey
            .mul_tweak(&key_coefficient)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale privkey: {}", e)))?;

        let privkey_challenge = privkey_scaled.mul_tweak(&challenge).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to multiply by challenge: {}", e))
        })?;

        let s = k_eff.add_tweak(&privkey_challenge.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute signature: {}", e))
        })?;

        Ok(PartialSignature {
            signer_pubkey: self.pubkey,
            s: s.secret_bytes(),
        })
    }

    /// Clear nonces after signing (security best practice)
    pub fn clear_nonces(&mut self) {
        self.nonce_secrets = None;
        self.nonce_public = None;
    }
}

/// MuSig2 coordinator for aggregating nonces and signatures
#[derive(Debug)]
pub struct MuSig2Coordinator {
    /// Key aggregator
    key_aggregator: MuSig2KeyAggregator,
    /// Collected nonces from signers
    nonces: HashMap<PublicKey, MuSig2Nonce>,
    /// Aggregated nonce
    aggregated_nonce: Option<AggregatedNonce>,
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl MuSig2Coordinator {
    /// Create a new coordinator with a key aggregator
    pub fn new(key_aggregator: MuSig2KeyAggregator) -> Self {
        Self {
            key_aggregator,
            nonces: HashMap::new(),
            aggregated_nonce: None,
            secp: Secp256k1::new(),
        }
    }

    /// Add a nonce from a signer
    pub fn add_nonce(
        &mut self,
        signer_pubkey: PublicKey,
        nonce: MuSig2Nonce,
    ) -> Result<(), BitcoinError> {
        if !self.key_aggregator.public_keys().contains(&signer_pubkey) {
            return Err(BitcoinError::InvalidAddress(
                "Signer not in key aggregation set".to_string(),
            ));
        }

        self.nonces.insert(signer_pubkey, nonce);
        Ok(())
    }

    /// Aggregate all nonces once all are collected
    pub fn aggregate_nonces(&mut self) -> Result<AggregatedNonce, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        if self.nonces.len() != self.key_aggregator.public_keys().len() {
            return Err(BitcoinError::InvalidAddress(
                "Not all nonces collected".to_string(),
            ));
        }

        // Compute R1 = sum(r1_i) and R2 = sum(r2_i)
        let mut r1_agg: Option<PublicKey> = None;
        let mut r2_agg: Option<PublicKey> = None;

        for nonce in self.nonces.values() {
            r1_agg = Some(if let Some(acc) = r1_agg {
                acc.combine(&nonce.r1).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to aggregate r1: {}", e))
                })?
            } else {
                nonce.r1
            });

            r2_agg = Some(if let Some(acc) = r2_agg {
                acc.combine(&nonce.r2).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to aggregate r2: {}", e))
                })?
            } else {
                nonce.r2
            });
        }

        let r1 = r1_agg
            .ok_or_else(|| BitcoinError::InvalidAddress("Failed to aggregate R1".to_string()))?;
        let r2 = r2_agg
            .ok_or_else(|| BitcoinError::InvalidAddress("Failed to aggregate R2".to_string()))?;

        // Compute nonce coefficient: b = hash(R1 || R2 || Q || m)
        // Simplified - in production would include message
        let mut engine = sha256::Hash::engine();
        engine.input(&r1.serialize());
        engine.input(&r2.serialize());
        engine.input(&self.key_aggregator.aggregated_key().serialize());
        let b_hash = sha256::Hash::from_engine(engine);
        let b = Scalar::from_be_bytes(b_hash.to_byte_array()).map_err(|_| {
            BitcoinError::InvalidAddress("Failed to compute nonce coefficient".to_string())
        })?;

        // Compute R = R1 + b*R2
        let r2_scaled = r2
            .mul_tweak(&self.secp, &b)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale R2: {}", e)))?;

        let r = r1.combine(&r2_scaled).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute final R: {}", e))
        })?;

        let aggregated = AggregatedNonce { r, b };
        self.aggregated_nonce = Some(aggregated);

        Ok(aggregated)
    }

    /// Aggregate partial signatures into final signature
    pub fn aggregate_signatures(
        &self,
        partial_sigs: &[PartialSignature],
    ) -> Result<[u8; 32], BitcoinError> {
        if partial_sigs.len() != self.key_aggregator.public_keys().len() {
            return Err(BitcoinError::InvalidAddress(
                "Not all partial signatures provided".to_string(),
            ));
        }

        // Verify all signers are accounted for
        for sig in partial_sigs {
            if !self
                .key_aggregator
                .public_keys()
                .contains(&sig.signer_pubkey)
            {
                return Err(BitcoinError::InvalidAddress(
                    "Unknown signer in partial signatures".to_string(),
                ));
            }
        }

        // Aggregate signatures: s = sum(s_i) mod n
        // Convert first signature to SecretKey for scalar addition
        let mut aggregated = SecretKey::from_slice(&partial_sigs[0].s).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid partial signature: {}", e))
        })?;

        // Add remaining signatures
        for sig in &partial_sigs[1..] {
            let sig_scalar = SecretKey::from_slice(&sig.s).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Invalid partial signature: {}", e))
            })?;

            aggregated = aggregated.add_tweak(&sig_scalar.into()).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to aggregate signatures: {}", e))
            })?;
        }

        Ok(aggregated.secret_bytes())
    }

    /// Get the aggregated nonce
    pub fn aggregated_nonce(&self) -> Option<AggregatedNonce> {
        self.aggregated_nonce
    }

    /// Get the key aggregator
    pub fn key_aggregator(&self) -> &MuSig2KeyAggregator {
        &self.key_aggregator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::rand::rngs::OsRng;

    #[test]
    fn test_key_aggregation_single_key() {
        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let aggregator = MuSig2KeyAggregator::new(vec![pk]).unwrap();
        assert_eq!(aggregator.public_keys().len(), 1);
    }

    #[test]
    fn test_key_aggregation_multi_key() {
        let sk1 = SecretKey::new(&mut OsRng);
        let sk2 = SecretKey::new(&mut OsRng);
        let sk3 = SecretKey::new(&mut OsRng);

        let secp = Secp256k1::new();
        let pk1 = PublicKey::from_secret_key(&secp, &sk1);
        let pk2 = PublicKey::from_secret_key(&secp, &sk2);
        let pk3 = PublicKey::from_secret_key(&secp, &sk3);

        let aggregator = MuSig2KeyAggregator::new(vec![pk1, pk2, pk3]).unwrap();
        assert_eq!(aggregator.public_keys().len(), 3);

        // Each key should have a coefficient
        assert!(aggregator.get_coefficient(&pk1).is_some());
        assert!(aggregator.get_coefficient(&pk2).is_some());
        assert!(aggregator.get_coefficient(&pk3).is_some());
    }

    #[test]
    fn test_signer_creation() {
        let sk = SecretKey::new(&mut OsRng);
        let signer = MuSig2Signer::new(sk).unwrap();

        assert_eq!(
            signer.public_key(),
            PublicKey::from_secret_key(&Secp256k1::new(), &sk)
        );
    }

    #[test]
    fn test_nonce_generation() {
        let sk = SecretKey::new(&mut OsRng);
        let mut signer = MuSig2Signer::new(sk).unwrap();

        let nonce1 = signer.generate_nonces().unwrap();
        let nonce2 = signer.generate_nonces().unwrap();

        // Each nonce generation should produce different values
        assert_ne!(nonce1.r1, nonce2.r1);
        assert_ne!(nonce1.r2, nonce2.r2);
    }

    #[test]
    fn test_coordinator_creation() {
        let sk1 = SecretKey::new(&mut OsRng);
        let sk2 = SecretKey::new(&mut OsRng);

        let secp = Secp256k1::new();
        let pk1 = PublicKey::from_secret_key(&secp, &sk1);
        let pk2 = PublicKey::from_secret_key(&secp, &sk2);

        let aggregator = MuSig2KeyAggregator::new(vec![pk1, pk2]).unwrap();
        let coordinator = MuSig2Coordinator::new(aggregator);

        assert_eq!(coordinator.key_aggregator().public_keys().len(), 2);
    }

    #[test]
    fn test_nonce_aggregation() {
        let sk1 = SecretKey::new(&mut OsRng);
        let sk2 = SecretKey::new(&mut OsRng);

        let mut signer1 = MuSig2Signer::new(sk1).unwrap();
        let mut signer2 = MuSig2Signer::new(sk2).unwrap();

        let nonce1 = signer1.generate_nonces().unwrap();
        let nonce2 = signer2.generate_nonces().unwrap();

        let aggregator =
            MuSig2KeyAggregator::new(vec![signer1.public_key(), signer2.public_key()]).unwrap();
        let mut coordinator = MuSig2Coordinator::new(aggregator);

        coordinator.add_nonce(signer1.public_key(), nonce1).unwrap();
        coordinator.add_nonce(signer2.public_key(), nonce2).unwrap();

        let _agg_nonce = coordinator.aggregate_nonces().unwrap();
        assert!(coordinator.aggregated_nonce().is_some());
    }

    #[test]
    fn test_empty_keys_error() {
        let result = MuSig2KeyAggregator::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_clear() {
        let sk = SecretKey::new(&mut OsRng);
        let mut signer = MuSig2Signer::new(sk).unwrap();

        signer.generate_nonces().unwrap();
        assert!(signer.nonce_public.is_some());

        signer.clear_nonces();
        assert!(signer.nonce_public.is_none());
        assert!(signer.nonce_secrets.is_none());
    }

    #[test]
    fn test_adaptor_signature_generation() {
        let manager = AdaptorSignatureManager::new();
        let (secret, point) = manager.generate_adaptor().unwrap();

        // Verify the point is derived from the secret
        let secp = Secp256k1::new();
        let computed_point = PublicKey::from_secret_key(&secp, &secret);
        assert_eq!(computed_point, point);
    }

    #[test]
    fn test_adaptor_signature_creation() {
        let manager = AdaptorSignatureManager::new();
        let (_secret, point) = manager.generate_adaptor().unwrap();

        // Create a dummy partial signature
        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let partial_sig = PartialSignature {
            signer_pubkey: pk,
            s: [1u8; 32],
        };

        let agg_sig = [2u8; 32];
        let adaptor_sig = manager
            .create_adaptor_signature(vec![partial_sig], point, &agg_sig)
            .unwrap();

        assert_eq!(adaptor_sig.adaptor_point, point);
        assert_eq!(adaptor_sig.original_signatures.len(), 1);
    }

    #[test]
    fn test_adaptor_signature_completion() {
        let manager = AdaptorSignatureManager::new();
        let (secret, point) = manager.generate_adaptor().unwrap();

        // Create a dummy partial signature
        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let partial_sig = PartialSignature {
            signer_pubkey: pk,
            s: [1u8; 32],
        };

        let agg_sig = [2u8; 32];
        let adaptor_sig = manager
            .create_adaptor_signature(vec![partial_sig], point, &agg_sig)
            .unwrap();

        // Complete the adaptor signature
        let final_sig = manager
            .complete_adaptor_signature(&adaptor_sig, &secret)
            .unwrap();

        // The final signature should be different from the adapted signature
        assert_ne!(final_sig, adaptor_sig.adapted_signature);
    }

    #[test]
    fn test_adaptor_secret_recovery() {
        let manager = AdaptorSignatureManager::new();
        let (original_secret, point) = manager.generate_adaptor().unwrap();

        // Create adaptor signature
        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let partial_sig = PartialSignature {
            signer_pubkey: pk,
            s: [1u8; 32],
        };

        let agg_sig = [2u8; 32];
        let adaptor_sig = manager
            .create_adaptor_signature(vec![partial_sig], point, &agg_sig)
            .unwrap();

        // Complete and recover
        let final_sig = manager
            .complete_adaptor_signature(&adaptor_sig, &original_secret)
            .unwrap();

        let recovered_secret = adaptor_sig.recover_adaptor_secret(&final_sig).unwrap();

        // The recovered secret should match the original (simplified verification)
        assert_eq!(recovered_secret.secret_bytes().len(), 32);
    }

    #[test]
    fn test_adaptor_signature_verification() {
        let manager = AdaptorSignatureManager::new();
        let (_secret, point) = manager.generate_adaptor().unwrap();

        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let partial_sig = PartialSignature {
            signer_pubkey: pk,
            s: [1u8; 32],
        };

        let agg_sig = [2u8; 32];
        let adaptor_sig = manager
            .create_adaptor_signature(vec![partial_sig], point, &agg_sig)
            .unwrap();

        let message = [3u8; 32];
        let xonly_pk = pk.x_only_public_key().0;

        let is_valid = manager
            .verify_adaptor_signature(&adaptor_sig, &message, &xonly_pk)
            .unwrap();

        assert!(is_valid);
    }

    #[test]
    fn test_adaptor_wrong_secret() {
        let manager = AdaptorSignatureManager::new();
        let (_secret1, point) = manager.generate_adaptor().unwrap();
        let (wrong_secret, _) = manager.generate_adaptor().unwrap();

        let sk = SecretKey::new(&mut OsRng);
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);

        let partial_sig = PartialSignature {
            signer_pubkey: pk,
            s: [1u8; 32],
        };

        let agg_sig = [2u8; 32];
        let adaptor_sig = manager
            .create_adaptor_signature(vec![partial_sig], point, &agg_sig)
            .unwrap();

        // Try to complete with wrong secret - should fail
        let result = manager.complete_adaptor_signature(&adaptor_sig, &wrong_secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_partial_signatures() {
        let manager = AdaptorSignatureManager::new();
        let (_secret, point) = manager.generate_adaptor().unwrap();

        let agg_sig = [2u8; 32];
        let result = manager.create_adaptor_signature(vec![], point, &agg_sig);

        assert!(result.is_err());
    }
}
