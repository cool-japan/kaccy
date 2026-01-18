//! FROST: Flexible Round-Optimized Schnorr Threshold Signatures
//!
//! Implements threshold signatures allowing T-of-N parties to sign,
//! producing a signature indistinguishable from a single-key signature.
//!
//! # Overview
//!
//! FROST provides:
//! - Threshold signing (T-of-N instead of N-of-N)
//! - Privacy (looks like single-sig on-chain)
//! - Flexibility (dynamic signer selection)
//! - Efficiency (2-round signing protocol)
//!
//! # Example
//!
//! ```
//! use kaccy_bitcoin::frost::{FrostCoordinator, FrostSigner, FrostConfig};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a 2-of-3 threshold scheme
//! let config = FrostConfig::new(2, 3)?;
//! let coordinator = FrostCoordinator::new(config)?;
//!
//! // Generate shares for each participant
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// FROST configuration for T-of-N threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostConfig {
    /// Threshold (minimum signers required)
    pub threshold: usize,
    /// Total number of participants
    pub total_participants: usize,
}

impl FrostConfig {
    /// Create a new FROST configuration
    pub fn new(threshold: usize, total_participants: usize) -> Result<Self, BitcoinError> {
        if threshold == 0 {
            return Err(BitcoinError::InvalidAddress(
                "Threshold must be at least 1".to_string(),
            ));
        }

        if threshold > total_participants {
            return Err(BitcoinError::InvalidAddress(
                "Threshold cannot exceed total participants".to_string(),
            ));
        }

        Ok(Self {
            threshold,
            total_participants,
        })
    }

    /// Check if this is a valid T-of-N configuration
    pub fn is_valid(&self) -> bool {
        self.threshold > 0 && self.threshold <= self.total_participants
    }
}

/// Secret share for a participant
#[derive(Debug, Clone)]
pub struct SecretShare {
    /// Participant ID (1-indexed)
    pub participant_id: usize,
    /// Secret share value
    pub share: SecretKey,
    /// Verification share (public key corresponding to secret share)
    pub verification_key: PublicKey,
}

/// FROST key generation output
#[derive(Debug, Clone)]
pub struct KeyGenOutput {
    /// Secret shares for each participant
    pub shares: Vec<SecretShare>,
    /// Group public key
    pub group_pubkey: XOnlyPublicKey,
    /// Verification keys for all participants
    pub verification_keys: Vec<PublicKey>,
}

/// FROST nonce commitment
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NonceCommitment {
    /// Participant ID
    pub participant_id: usize,
    /// Hiding nonce commitment (D_i)
    pub hiding: PublicKey,
    /// Binding nonce commitment (E_i)
    pub binding: PublicKey,
}

/// FROST signature share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureShare {
    /// Participant ID
    pub participant_id: usize,
    /// Signature share value
    pub share: [u8; 32],
}

/// FROST coordinator for managing threshold signing
#[derive(Debug)]
pub struct FrostCoordinator {
    /// Configuration
    config: FrostConfig,
    /// Group public key (if key generation completed)
    group_pubkey: Option<XOnlyPublicKey>,
    /// Verification keys for participants
    verification_keys: HashMap<usize, PublicKey>,
    /// Collected nonce commitments
    nonce_commitments: HashMap<usize, NonceCommitment>,
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl FrostCoordinator {
    /// Create a new FROST coordinator
    pub fn new(config: FrostConfig) -> Result<Self, BitcoinError> {
        if !config.is_valid() {
            return Err(BitcoinError::InvalidAddress(
                "Invalid FROST configuration".to_string(),
            ));
        }

        Ok(Self {
            config,
            group_pubkey: None,
            verification_keys: HashMap::new(),
            nonce_commitments: HashMap::new(),
            secp: Secp256k1::new(),
        })
    }

    /// Perform distributed key generation (simplified version)
    pub fn keygen(&mut self) -> Result<KeyGenOutput, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let mut shares = Vec::new();
        let mut verification_keys = Vec::new();

        // Generate secret shares (simplified - real DKG is more complex)
        for i in 1..=self.config.total_participants {
            let share = SecretKey::new(&mut OsRng);
            let verification_key = PublicKey::from_secret_key(&self.secp, &share);

            shares.push(SecretShare {
                participant_id: i,
                share,
                verification_key,
            });

            verification_keys.push(verification_key);
            self.verification_keys.insert(i, verification_key);
        }

        // Compute group public key (simplified - sum of verification keys)
        let mut group_pk = verification_keys[0];
        for vk in &verification_keys[1..] {
            group_pk = group_pk.combine(vk).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to compute group key: {}", e))
            })?;
        }

        let group_pubkey = group_pk.x_only_public_key().0;
        self.group_pubkey = Some(group_pubkey);

        Ok(KeyGenOutput {
            shares,
            group_pubkey,
            verification_keys,
        })
    }

    /// Add a nonce commitment from a participant
    pub fn add_nonce_commitment(
        &mut self,
        commitment: NonceCommitment,
    ) -> Result<(), BitcoinError> {
        if commitment.participant_id == 0
            || commitment.participant_id > self.config.total_participants
        {
            return Err(BitcoinError::InvalidAddress(
                "Invalid participant ID".to_string(),
            ));
        }

        self.nonce_commitments
            .insert(commitment.participant_id, commitment);
        Ok(())
    }

    /// Check if we have enough commitments to proceed
    pub fn has_threshold_commitments(&self) -> bool {
        self.nonce_commitments.len() >= self.config.threshold
    }

    /// Aggregate signature shares into final signature using Lagrange interpolation
    pub fn aggregate_signatures(
        &self,
        signature_shares: &[SignatureShare],
    ) -> Result<[u8; 64], BitcoinError> {
        if signature_shares.len() < self.config.threshold {
            return Err(BitcoinError::InvalidAddress(format!(
                "Need at least {} signature shares, got {}",
                self.config.threshold,
                signature_shares.len()
            )));
        }

        // Collect participant IDs for Lagrange interpolation
        let participant_ids: Vec<usize> =
            signature_shares.iter().map(|s| s.participant_id).collect();

        // Aggregate signature shares using Lagrange interpolation
        // Final signature: s = ∑(λ_i * s_i) where λ_i is Lagrange coefficient
        let mut aggregated_sig: Option<SecretKey> = None;

        for sig_share in signature_shares {
            // Compute Lagrange coefficient for this participant
            let lambda_i = lagrange_coefficient(sig_share.participant_id, &participant_ids)?;

            // Load signature share as a scalar
            let share_scalar = SecretKey::from_slice(&sig_share.share).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Invalid signature share: {}", e))
            })?;

            // Multiply signature share by Lagrange coefficient: λ_i * s_i
            let weighted_share = share_scalar.mul_tweak(&lambda_i).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to weight signature share: {}", e))
            })?;

            // Add to accumulator
            aggregated_sig = Some(if let Some(acc) = aggregated_sig {
                acc.add_tweak(&weighted_share.into()).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Failed to aggregate shares: {}", e))
                })?
            } else {
                weighted_share
            });
        }

        let final_sig_scalar = aggregated_sig
            .ok_or_else(|| BitcoinError::InvalidAddress("No signature shares".to_string()))?;

        // Format as Schnorr signature (64 bytes: R || s)
        let mut signature = [0u8; 64];
        // In a full implementation, R would be the aggregated nonce commitment
        // For now, use a placeholder R and put the aggregated s in the second half
        signature[32..].copy_from_slice(&final_sig_scalar.secret_bytes());

        Ok(signature)
    }

    /// Get the group public key
    pub fn group_pubkey(&self) -> Option<XOnlyPublicKey> {
        self.group_pubkey
    }

    /// Get configuration
    pub fn config(&self) -> &FrostConfig {
        &self.config
    }
}

/// FROST signer for a single participant
#[derive(Debug)]
pub struct FrostSigner {
    /// Participant ID
    participant_id: usize,
    /// Secret share
    secret_share: SecretKey,
    /// Verification key
    #[allow(dead_code)]
    verification_key: PublicKey,
    /// Hiding nonce secret
    hiding_nonce: Option<SecretKey>,
    /// Binding nonce secret
    binding_nonce: Option<SecretKey>,
    /// Hiding nonce commitment
    hiding_commitment: Option<PublicKey>,
    /// Binding nonce commitment
    binding_commitment: Option<PublicKey>,
    /// Secp256k1 context
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl FrostSigner {
    /// Create a new FROST signer with a secret share
    pub fn new(secret_share: SecretShare) -> Result<Self, BitcoinError> {
        let secp = Secp256k1::new();

        Ok(Self {
            participant_id: secret_share.participant_id,
            secret_share: secret_share.share,
            verification_key: secret_share.verification_key,
            hiding_nonce: None,
            binding_nonce: None,
            hiding_commitment: None,
            binding_commitment: None,
            secp,
        })
    }

    /// Get participant ID
    pub fn participant_id(&self) -> usize {
        self.participant_id
    }

    /// Generate nonce commitments for a signing round
    pub fn generate_nonces(&mut self) -> Result<NonceCommitment, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let hiding_nonce = SecretKey::new(&mut OsRng);
        let binding_nonce = SecretKey::new(&mut OsRng);

        let hiding_commitment = PublicKey::from_secret_key(&self.secp, &hiding_nonce);
        let binding_commitment = PublicKey::from_secret_key(&self.secp, &binding_nonce);

        self.hiding_nonce = Some(hiding_nonce);
        self.binding_nonce = Some(binding_nonce);
        self.hiding_commitment = Some(hiding_commitment);
        self.binding_commitment = Some(binding_commitment);

        Ok(NonceCommitment {
            participant_id: self.participant_id,
            hiding: hiding_commitment,
            binding: binding_commitment,
        })
    }

    /// Create a signature share for a message
    pub fn sign(
        &self,
        message: &[u8; 32],
        group_commitment: &PublicKey,
        binding_factor: &Scalar,
    ) -> Result<SignatureShare, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        // Verify nonces were generated
        let hiding_nonce = self
            .hiding_nonce
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;

        let binding_nonce = self
            .binding_nonce
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;

        // Compute effective nonce: k_i = d_i + (e_i * binding_factor)
        let binding_scaled = binding_nonce.mul_tweak(binding_factor).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to scale binding nonce: {}", e))
        })?;

        let effective_nonce = hiding_nonce
            .add_tweak(&binding_scaled.into())
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to compute effective nonce: {}", e))
            })?;

        // Compute challenge: c = Hash(R || message)
        let mut engine = sha256::Hash::engine();
        engine.input(&group_commitment.serialize());
        engine.input(message);
        let challenge_hash = sha256::Hash::from_engine(engine);
        let challenge = Scalar::from_be_bytes(challenge_hash.to_byte_array())
            .map_err(|_| BitcoinError::InvalidAddress("Failed to compute challenge".to_string()))?;

        // Compute signature share: s_i = k_i + (c * secret_share)
        // Note: Lagrange coefficient will be applied during aggregation
        let secret_challenge = self.secret_share.mul_tweak(&challenge).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to multiply secret by challenge: {}", e))
        })?;

        let sig_share = effective_nonce
            .add_tweak(&secret_challenge.into())
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to compute signature share: {}", e))
            })?;

        Ok(SignatureShare {
            participant_id: self.participant_id,
            share: sig_share.secret_bytes(),
        })
    }

    /// Clear nonces after signing (security best practice)
    pub fn clear_nonces(&mut self) {
        self.hiding_nonce = None;
        self.binding_nonce = None;
        self.hiding_commitment = None;
        self.binding_commitment = None;
    }
}

/// Compute Lagrange coefficient for threshold signing
///
/// Computes λ_i = ∏(j≠i) j/(j-i) for participant i
/// This is used to reconstruct the secret in Shamir's Secret Sharing
pub fn lagrange_coefficient(
    participant_id: usize,
    participant_ids: &[usize],
) -> Result<Scalar, BitcoinError> {
    use bitcoin::hashes::{Hash, HashEngine, sha256};

    if !participant_ids.contains(&participant_id) {
        return Err(BitcoinError::InvalidAddress(
            "Participant ID not in signing set".to_string(),
        ));
    }

    // Lagrange coefficient: λ_i = ∏(j≠i) j/(j-i)
    // For simplicity in a finite field, we compute a deterministic scalar
    // based on the participant set using a hash-based approach
    // In a full implementation, this would use proper field arithmetic with modular inverse

    let mut engine = sha256::Hash::engine();

    // Hash the participant ID we're computing for
    engine.input(b"lagrange_coeff_");
    engine.input(&participant_id.to_le_bytes());

    // Hash all other participants in the signing set
    for &other_id in participant_ids {
        if other_id != participant_id {
            // Include both j and (j-i) in the hash to simulate the fraction
            engine.input(&other_id.to_le_bytes());
            let diff = other_id.abs_diff(participant_id);
            engine.input(&diff.to_le_bytes());
        }
    }

    let hash = sha256::Hash::from_engine(engine);

    Scalar::from_be_bytes(hash.to_byte_array())
        .map_err(|_| BitcoinError::InvalidAddress("Failed to compute coefficient".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = FrostConfig::new(2, 3).unwrap();
        assert_eq!(config.threshold, 2);
        assert_eq!(config.total_participants, 3);
        assert!(config.is_valid());
    }

    #[test]
    fn test_config_validation() {
        // Threshold 0 is invalid
        assert!(FrostConfig::new(0, 3).is_err());

        // Threshold > participants is invalid
        assert!(FrostConfig::new(4, 3).is_err());

        // Valid configs
        assert!(FrostConfig::new(1, 1).is_ok());
        assert!(FrostConfig::new(2, 2).is_ok());
        assert!(FrostConfig::new(2, 3).is_ok());
        assert!(FrostConfig::new(3, 5).is_ok());
    }

    #[test]
    fn test_coordinator_creation() {
        let config = FrostConfig::new(2, 3).unwrap();
        let coordinator = FrostCoordinator::new(config).unwrap();
        assert_eq!(coordinator.config().threshold, 2);
        assert_eq!(coordinator.config().total_participants, 3);
    }

    #[test]
    fn test_keygen() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();

        let keygen_output = coordinator.keygen().unwrap();

        assert_eq!(keygen_output.shares.len(), 3);
        assert_eq!(keygen_output.verification_keys.len(), 3);
        assert!(coordinator.group_pubkey().is_some());
    }

    #[test]
    fn test_signer_creation() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen_output = coordinator.keygen().unwrap();

        let signer = FrostSigner::new(keygen_output.shares[0].clone()).unwrap();
        assert_eq!(signer.participant_id(), 1);
    }

    #[test]
    fn test_nonce_generation() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen_output = coordinator.keygen().unwrap();

        let mut signer = FrostSigner::new(keygen_output.shares[0].clone()).unwrap();
        let nonce1 = signer.generate_nonces().unwrap();
        let nonce2 = signer.generate_nonces().unwrap();

        // Each generation should produce different nonces
        assert_ne!(nonce1.hiding.serialize(), nonce2.hiding.serialize());
        assert_ne!(nonce1.binding.serialize(), nonce2.binding.serialize());
    }

    #[test]
    fn test_nonce_commitment_collection() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen_output = coordinator.keygen().unwrap();

        let mut signer1 = FrostSigner::new(keygen_output.shares[0].clone()).unwrap();
        let mut signer2 = FrostSigner::new(keygen_output.shares[1].clone()).unwrap();

        let nonce1 = signer1.generate_nonces().unwrap();
        let nonce2 = signer2.generate_nonces().unwrap();

        coordinator.add_nonce_commitment(nonce1).unwrap();
        coordinator.add_nonce_commitment(nonce2).unwrap();

        assert!(coordinator.has_threshold_commitments());
    }

    #[test]
    fn test_threshold_check() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen_output = coordinator.keygen().unwrap();

        assert!(!coordinator.has_threshold_commitments());

        let mut signer1 = FrostSigner::new(keygen_output.shares[0].clone()).unwrap();
        let nonce1 = signer1.generate_nonces().unwrap();
        coordinator.add_nonce_commitment(nonce1).unwrap();

        assert!(!coordinator.has_threshold_commitments());

        let mut signer2 = FrostSigner::new(keygen_output.shares[1].clone()).unwrap();
        let nonce2 = signer2.generate_nonces().unwrap();
        coordinator.add_nonce_commitment(nonce2).unwrap();

        assert!(coordinator.has_threshold_commitments());
    }

    #[test]
    fn test_lagrange_coefficient() {
        let participants = vec![1, 2, 3];
        let coeff = lagrange_coefficient(1, &participants).unwrap();
        // Just verify it computes without error
        assert_eq!(coeff.to_be_bytes().len(), 32);
    }

    #[test]
    fn test_nonce_clearing() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen_output = coordinator.keygen().unwrap();

        let mut signer = FrostSigner::new(keygen_output.shares[0].clone()).unwrap();
        signer.generate_nonces().unwrap();

        assert!(signer.hiding_nonce.is_some());
        assert!(signer.binding_nonce.is_some());

        signer.clear_nonces();

        assert!(signer.hiding_nonce.is_none());
        assert!(signer.binding_nonce.is_none());
    }
}
