//! FROST: Flexible Round-Optimized Schnorr Threshold Signatures
//!
//! Implements real threshold Schnorr signatures (RFC 9591 / BIP-340 style) over
//! secp256k1, allowing any `t`-of-`n` subset of parties to jointly produce a
//! signature that is indistinguishable from an ordinary single-key BIP-340
//! Schnorr signature and verifies under the group x-only public key.
//!
//! # What is implemented (real cryptography)
//!
//! - **Verifiable key generation** via a *trusted dealer* using Shamir secret
//!   sharing over the secp256k1 scalar field `GF(n)` together with Feldman VSS
//!   commitments `C_j = a_j·G`. Every participant can verify their share against
//!   the published commitments: `s_i·G == Σ_j i^j·C_j`. The group secret is split
//!   with a degree `t-1` polynomial; reconstructing it requires `t` shares.
//! - **BIP-340 even-Y normalization** of the group key and all shares, so the
//!   aggregate signature is a valid x-only Schnorr signature.
//! - **Two-round FROST signing**: hiding/binding nonce commitments, per-signer
//!   binding factors, aggregation of the *real* nonce commitments into the group
//!   nonce `R` (always a valid curve point — never a zeroed placeholder), the
//!   BIP-340 challenge `e = H_tagged("BIP0340/challenge", R.x || Q.x || m)`,
//!   Lagrange-weighted partial signatures, and aggregation to `(R.x, z)`.
//! - **Partial-signature verification**, allowing the coordinator to detect a
//!   malformed or tampered share before aggregation.
//!
//! # Follow-up work (intentionally not implemented here)
//!
//! The key generation here uses a *single trusted dealer*. A full distributed
//! key generation (DKG) — e.g. Pedersen DKG with per-participant polynomials,
//! complaint rounds, and proofs-of-possession so that no single party ever holds
//! the group secret — is a natural follow-up. The signing protocol below is
//! independent of how the shares were produced and works unchanged with DKG
//! output.
//!
//! # Example
//!
//! ```
//! use kaccy_bitcoin::frost::{FrostCoordinator, FrostSigner, FrostConfig};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a 2-of-3 threshold scheme.
//! let config = FrostConfig::new(2, 3)?;
//! let mut coordinator = FrostCoordinator::new(config)?;
//!
//! // Trusted-dealer key generation produces verifiable shares + a group key.
//! let keygen = coordinator.keygen()?;
//! let _group_key = keygen.group_pubkey;
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::secp256k1::{
    Message, PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey, schnorr,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// FROST configuration for `t`-of-`n` threshold.
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

/// Secret share for a participant.
#[derive(Debug, Clone)]
pub struct SecretShare {
    /// Participant ID (1-indexed, used as the Shamir evaluation point `x`)
    pub participant_id: usize,
    /// Secret share value `s_i = f(i) mod n`
    pub share: SecretKey,
    /// Verification share `Y_i = s_i·G`
    pub verification_key: PublicKey,
}

/// FROST key generation output.
#[derive(Debug, Clone)]
pub struct KeyGenOutput {
    /// Secret shares for each participant
    pub shares: Vec<SecretShare>,
    /// Group public key (x-only, BIP-340 even-Y normalized)
    pub group_pubkey: XOnlyPublicKey,
    /// Verification keys `Y_i` for all participants
    pub verification_keys: Vec<PublicKey>,
    /// Feldman VSS commitments `C_j = a_j·G` for `j = 0..threshold`.
    ///
    /// `commitments[0]` is the group public key in (full, even-Y) form; the
    /// remaining entries commit to the secret polynomial coefficients and let
    /// any participant verify their share with [`verify_share`].
    pub commitments: Vec<PublicKey>,
}

/// FROST nonce commitment broadcast in signing round 1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NonceCommitment {
    /// Participant ID
    pub participant_id: usize,
    /// Hiding nonce commitment `D_i = d_i·G`
    pub hiding: PublicKey,
    /// Binding nonce commitment `E_i = e_i·G`
    pub binding: PublicKey,
}

/// FROST signature share produced by a single signer in round 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureShare {
    /// Participant ID
    pub participant_id: usize,
    /// Signature share value `z_i` (big-endian scalar)
    pub share: [u8; 32],
}

/// FROST coordinator for managing threshold signing.
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

    /// Perform trusted-dealer key generation with Shamir secret sharing and
    /// Feldman VSS commitments.
    ///
    /// The dealer samples a uniform group secret `a_0` and a degree `t-1`
    /// polynomial `f(x) = a_0 + a_1·x + … + a_{t-1}·x^{t-1}` over `GF(n)`,
    /// publishes Feldman commitments `C_j = a_j·G`, and hands participant `i`
    /// the share `s_i = f(i) mod n`. The polynomial is negated as a whole if
    /// required so that the group public key `a_0·G` has even Y, keeping the
    /// aggregate signature a valid BIP-340 x-only signature. Every share is
    /// verifiable against the commitments via [`verify_share`].
    pub fn keygen(&mut self) -> Result<KeyGenOutput, BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let threshold = self.config.threshold;
        let total = self.config.total_participants;

        // 1. Sample the secret polynomial coefficients a_0..a_{t-1} in [1, n).
        //    SecretKey::new guarantees a valid non-zero scalar.
        let mut coeffs: Vec<Fp> = Vec::with_capacity(threshold);
        for _ in 0..threshold {
            let sk = SecretKey::new(&mut OsRng);
            coeffs.push(Fp::from_secret_key(&sk));
        }

        // 2. BIP-340 even-Y normalization of the whole polynomial. The group
        //    public key is a_0·G; if its Y is odd, negate every coefficient
        //    (which negates f, hence the secret and all shares) so that the
        //    group key becomes even-Y. f(x) and -f(x) interpolate to a_0 and
        //    -a_0 respectively, so negating all coefficients is consistent.
        let secret_a0 = coeffs[0].to_secret_key()?;
        let (_group_xonly_pre, parity) = secret_a0.x_only_public_key(&self.secp);
        if parity == bitcoin::secp256k1::Parity::Odd {
            for c in coeffs.iter_mut() {
                *c = c.negate();
            }
        }

        // 3. Feldman commitments C_j = a_j·G.
        let mut commitments: Vec<PublicKey> = Vec::with_capacity(threshold);
        for c in &coeffs {
            let sk = c.to_secret_key()?;
            commitments.push(PublicKey::from_secret_key(&self.secp, &sk));
        }

        // 4. Evaluate shares s_i = f(i) and verification keys Y_i = s_i·G.
        let mut shares = Vec::with_capacity(total);
        let mut verification_keys = Vec::with_capacity(total);
        self.verification_keys.clear();
        for i in 1..=total {
            let x = Fp::from_u64(i as u64);
            let s_i = Fp::eval_poly(&coeffs, &x);
            let share_sk = s_i.to_secret_key()?;
            let verification_key = PublicKey::from_secret_key(&self.secp, &share_sk);

            shares.push(SecretShare {
                participant_id: i,
                share: share_sk,
                verification_key,
            });
            verification_keys.push(verification_key);
            self.verification_keys.insert(i, verification_key);
        }

        // 5. Group public key is commitments[0] = a_0·G (now even-Y).
        let group_full = commitments[0];
        let (group_pubkey, group_parity) = group_full.x_only_public_key();
        debug_assert_eq!(group_parity, bitcoin::secp256k1::Parity::Even);
        self.group_pubkey = Some(group_pubkey);

        Ok(KeyGenOutput {
            shares,
            group_pubkey,
            verification_keys,
            commitments,
        })
    }

    /// Get the group public key
    pub fn group_pubkey(&self) -> Option<XOnlyPublicKey> {
        self.group_pubkey
    }

    /// Get configuration
    pub fn config(&self) -> &FrostConfig {
        &self.config
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

    /// Verify a participant's secret share against the Feldman VSS commitments.
    ///
    /// Returns `Ok(true)` iff `s_i·G == Σ_j i^j·C_j`. This catches a corrupted
    /// or maliciously substituted share at distribution time.
    pub fn verify_share(
        &self,
        share: &SecretShare,
        commitments: &[PublicKey],
    ) -> Result<bool, BitcoinError> {
        if commitments.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "No Feldman commitments provided".to_string(),
            ));
        }
        feldman_verify(&self.secp, share.participant_id, &share.share, commitments)
    }

    /// Aggregate signature shares into a final 64-byte BIP-340 Schnorr signature.
    ///
    /// Recomputes the group nonce `R` from the supplied round-1 commitments
    /// (so the signature carries the *real* nonce point), aggregates the partial
    /// signatures `z = Σ z_i mod n`, assembles `(R.x, z)`, and verifies the
    /// result against the group x-only key before returning it. A tampered or
    /// inconsistent partial signature therefore causes this call to fail rather
    /// than emit an invalid signature.
    pub fn aggregate_signatures(
        &self,
        message: &[u8; 32],
        commitments: &[NonceCommitment],
        signature_shares: &[SignatureShare],
    ) -> Result<[u8; 64], BitcoinError> {
        if signature_shares.len() < self.config.threshold {
            return Err(BitcoinError::InvalidAddress(format!(
                "Need at least {} signature shares, got {}",
                self.config.threshold,
                signature_shares.len()
            )));
        }

        let group_pubkey = self.group_pubkey.ok_or_else(|| {
            BitcoinError::InvalidAddress("Key generation not completed".to_string())
        })?;

        // Recompute the group nonce R from the real commitments.
        let (group_nonce, _r_parity) =
            compute_group_nonce(&self.secp, commitments, &group_pubkey, message)?;
        let (r_xonly, _) = group_nonce.x_only_public_key();

        // Aggregate partial signatures modulo the curve order.
        let mut z = Fp::ZERO;
        for sig_share in signature_shares {
            let z_i = Fp::from_be_bytes(&sig_share.share)?;
            z = z.add(&z_i);
        }

        // Assemble the 64-byte signature (R.x || z) and verify under the group key.
        let mut signature = [0u8; 64];
        signature[..32].copy_from_slice(&r_xonly.serialize());
        signature[32..].copy_from_slice(&z.to_be_bytes());

        let sig = schnorr::Signature::from_slice(&signature).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Malformed aggregated signature: {e}"))
        })?;
        let msg = Message::from_digest(*message);
        self.secp
            .verify_schnorr(&sig, &msg, &group_pubkey)
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!(
                    "Aggregated signature failed verification (tampered or inconsistent share): {e}"
                ))
            })?;

        Ok(signature)
    }
}

/// FROST signer for a single participant.
#[derive(Debug)]
pub struct FrostSigner {
    /// Participant ID
    participant_id: usize,
    /// Secret share `s_i`
    secret_share: SecretKey,
    /// Verification key `Y_i`
    #[allow(dead_code)]
    verification_key: PublicKey,
    /// Hiding nonce secret `d_i`
    hiding_nonce: Option<SecretKey>,
    /// Binding nonce secret `e_i`
    binding_nonce: Option<SecretKey>,
    /// Hiding nonce commitment `D_i`
    hiding_commitment: Option<PublicKey>,
    /// Binding nonce commitment `E_i`
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

    /// Generate hiding and binding nonce commitments for a signing round.
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

    /// Produce this signer's partial signature (FROST round 2).
    ///
    /// `signing_commitments` is the full set of round-1 commitments for the
    /// active signing set, `group_pubkey` the group x-only key, and `message`
    /// the 32-byte digest to sign. The signer computes its binding factor, the
    /// group nonce `R`, the BIP-340 challenge `e`, its Lagrange coefficient over
    /// the signer set, and the partial signature `z_i = k_i + e·λ_i·s_i`,
    /// where `k_i = d_i + ρ_i·e_i` is sign-adjusted so that the aggregate uses
    /// an even-Y `R`.
    pub fn sign(
        &self,
        message: &[u8; 32],
        signing_commitments: &[NonceCommitment],
        group_pubkey: &XOnlyPublicKey,
    ) -> Result<SignatureShare, BitcoinError> {
        let hiding_nonce = self
            .hiding_nonce
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;
        let binding_nonce = self
            .binding_nonce
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;

        if !signing_commitments
            .iter()
            .any(|c| c.participant_id == self.participant_id)
        {
            return Err(BitcoinError::InvalidAddress(
                "This signer is not part of the provided signing set".to_string(),
            ));
        }

        // Group nonce R and its parity over the active signer set.
        let (group_nonce, r_parity) =
            compute_group_nonce(&self.secp, signing_commitments, group_pubkey, message)?;
        let (r_xonly, _) = group_nonce.x_only_public_key();

        // This signer's binding factor ρ_i.
        let rho_i = binding_factor(signing_commitments, group_pubkey, message, self.participant_id)?;

        // Effective nonce k_i = d_i + ρ_i·e_i (as a field element).
        let d_i = Fp::from_secret_key(&hiding_nonce);
        let e_i = Fp::from_secret_key(&binding_nonce);
        let rho = Fp::from_scalar(&rho_i);
        let mut k_i = d_i.add(&e_i.mul(&rho));

        // If the aggregate R has odd Y, BIP-340 requires the nonce that yields
        // an even-Y R; negate the effective nonce so R.x stays consistent.
        if r_parity == bitcoin::secp256k1::Parity::Odd {
            k_i = k_i.negate();
        }

        // BIP-340 challenge e = H_tagged("BIP0340/challenge", R.x || Q.x || m).
        let e = bip340_challenge(&r_xonly, group_pubkey, message)?;

        // Lagrange coefficient over the active signer set.
        let signer_ids: Vec<usize> =
            signing_commitments.iter().map(|c| c.participant_id).collect();
        let lambda_i = lagrange_coefficient(self.participant_id, &signer_ids)?;
        let lambda = Fp::from_scalar(&lambda_i);

        // Partial signature z_i = k_i + e·λ_i·s_i.
        let s_i = Fp::from_secret_key(&self.secret_share);
        let z_i = k_i.add(&e.mul(&lambda).mul(&s_i));

        Ok(SignatureShare {
            participant_id: self.participant_id,
            share: z_i.to_be_bytes(),
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

/// Verify a single FROST partial signature `z_i` against the public data.
///
/// Checks `z_i·G == R_i' + e·λ_i·Y_i`, where `R_i = D_i + ρ_i·E_i` is this
/// signer's nonce contribution (negated when the aggregate `R` has odd Y).
/// Returns `Ok(true)` for a well-formed share and `Ok(false)` for a tampered
/// one, allowing the coordinator to attribute blame before aggregation.
pub fn verify_partial_signature(
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    signature_share: &SignatureShare,
    signing_commitments: &[NonceCommitment],
    verification_key: &PublicKey,
    group_pubkey: &XOnlyPublicKey,
    message: &[u8; 32],
) -> Result<bool, BitcoinError> {
    let participant_id = signature_share.participant_id;
    let commitment = signing_commitments
        .iter()
        .find(|c| c.participant_id == participant_id)
        .ok_or_else(|| {
            BitcoinError::InvalidAddress("Commitment for signer not found".to_string())
        })?;

    let (group_nonce, r_parity) =
        compute_group_nonce(secp, signing_commitments, group_pubkey, message)?;
    let _ = group_nonce;

    // Left-hand side: z_i·G.
    let z_i = Fp::from_be_bytes(&signature_share.share)?;
    let lhs = match z_i.to_secret_key() {
        Ok(sk) => Some(PublicKey::from_secret_key(secp, &sk)),
        // z_i == 0 means lhs is the identity (point at infinity).
        Err(_) => None,
    };

    // R_i = D_i + ρ_i·E_i (sign-adjusted to match the aggregate R parity).
    let rho_i = binding_factor(signing_commitments, group_pubkey, message, participant_id)?;
    let rho_e = commitment.binding.mul_tweak(secp, &rho_i).map_err(|e| {
        BitcoinError::InvalidAddress(format!("Failed to scale binding commitment: {e}"))
    })?;
    let mut r_i = commitment.hiding.combine(&rho_e).map_err(|e| {
        BitcoinError::InvalidAddress(format!("Failed to combine nonce commitments: {e}"))
    })?;
    if r_parity == bitcoin::secp256k1::Parity::Odd {
        r_i = r_i.negate(secp);
    }

    // e·λ_i·Y_i.
    let (r_xonly, _) = {
        let (gn, _) = compute_group_nonce(secp, signing_commitments, group_pubkey, message)?;
        gn.x_only_public_key()
    };
    let e = bip340_challenge(&r_xonly, group_pubkey, message)?;
    let signer_ids: Vec<usize> = signing_commitments.iter().map(|c| c.participant_id).collect();
    let lambda_i = lagrange_coefficient(participant_id, &signer_ids)?;
    let e_lambda = e.mul(&Fp::from_scalar(&lambda_i));
    let e_lambda_scalar = e_lambda.to_scalar()?;
    let challenge_term = verification_key.mul_tweak(secp, &e_lambda_scalar).map_err(|err| {
        BitcoinError::InvalidAddress(format!("Failed to scale verification key: {err}"))
    })?;

    // Right-hand side: R_i + e·λ_i·Y_i.
    let rhs = r_i.combine(&challenge_term).map_err(|err| {
        BitcoinError::InvalidAddress(format!("Failed to combine RHS terms: {err}"))
    })?;

    match lhs {
        Some(lhs) => Ok(lhs == rhs),
        // lhs is the identity; equality cannot hold for a finite rhs.
        None => Ok(false),
    }
}

/// Compute the binding factor `ρ_i` for a participant.
///
/// `ρ_i = H_tagged("FROST/secp256k1/v1/rho",
///                  Q.x || msg || encode(all commitments) || i) mod n`.
/// The commitments are sorted by participant id so all parties derive the same
/// binding factors regardless of input ordering.
fn binding_factor(
    signing_commitments: &[NonceCommitment],
    group_pubkey: &XOnlyPublicKey,
    message: &[u8; 32],
    participant_id: usize,
) -> Result<Scalar, BitcoinError> {
    use bitcoin::hashes::{Hash, HashEngine, sha256};

    let mut sorted: Vec<&NonceCommitment> = signing_commitments.iter().collect();
    sorted.sort_by_key(|c| c.participant_id);

    let tag = sha256::Hash::hash(b"FROST/secp256k1/v1/rho");
    let mut engine = sha256::Hash::engine();
    engine.input(tag.as_ref());
    engine.input(tag.as_ref());
    engine.input(&group_pubkey.serialize());
    engine.input(message);
    for c in &sorted {
        engine.input(&(c.participant_id as u64).to_be_bytes());
        engine.input(&c.hiding.serialize());
        engine.input(&c.binding.serialize());
    }
    engine.input(&(participant_id as u64).to_be_bytes());
    let digest = sha256::Hash::from_engine(engine);

    Fp::from_be_bytes(&digest.to_byte_array())?.to_scalar_allow_zero()
}

/// Aggregate the per-signer nonce commitments into the group nonce
/// `R = Σ_i (D_i + ρ_i·E_i)` and return it together with its Y parity.
fn compute_group_nonce(
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    signing_commitments: &[NonceCommitment],
    group_pubkey: &XOnlyPublicKey,
    message: &[u8; 32],
) -> Result<(PublicKey, bitcoin::secp256k1::Parity), BitcoinError> {
    if signing_commitments.is_empty() {
        return Err(BitcoinError::InvalidAddress(
            "No signing commitments provided".to_string(),
        ));
    }

    let mut acc: Option<PublicKey> = None;
    for c in signing_commitments {
        let rho_i = binding_factor(signing_commitments, group_pubkey, message, c.participant_id)?;
        let rho_e = c.binding.mul_tweak(secp, &rho_i).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to scale binding commitment: {e}"))
        })?;
        let contribution = c.hiding.combine(&rho_e).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to combine nonce commitments: {e}"))
        })?;
        acc = Some(match acc {
            Some(a) => a.combine(&contribution).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to aggregate group nonce: {e}"))
            })?,
            None => contribution,
        });
    }

    let r = acc.ok_or_else(|| {
        BitcoinError::InvalidAddress("Group nonce aggregation produced no point".to_string())
    })?;
    let (_, parity) = r.x_only_public_key();
    Ok((r, parity))
}

/// BIP-340 challenge `e = int(H_tagged("BIP0340/challenge", R.x || P.x || m)) mod n`.
fn bip340_challenge(
    r_xonly: &XOnlyPublicKey,
    group_pubkey: &XOnlyPublicKey,
    message: &[u8; 32],
) -> Result<Fp, BitcoinError> {
    use bitcoin::hashes::{Hash, HashEngine, sha256};

    let tag = sha256::Hash::hash(b"BIP0340/challenge");
    let mut engine = sha256::Hash::engine();
    engine.input(tag.as_ref());
    engine.input(tag.as_ref());
    engine.input(&r_xonly.serialize());
    engine.input(&group_pubkey.serialize());
    engine.input(message);
    let digest = sha256::Hash::from_engine(engine);
    Fp::from_be_bytes(&digest.to_byte_array())
}

/// Compute the Lagrange coefficient `λ_i` for a participant over a signing set.
///
/// `λ_i = ∏_{j≠i} j / (j - i) (mod n)`, evaluated at `x = 0` so that
/// `Σ_i λ_i · f(i) = f(0)` recovers the group secret. Uses exact modular
/// arithmetic over `GF(n)` (including modular inverse), not a heuristic.
pub fn lagrange_coefficient(
    participant_id: usize,
    participant_ids: &[usize],
) -> Result<Scalar, BitcoinError> {
    if !participant_ids.contains(&participant_id) {
        return Err(BitcoinError::InvalidAddress(
            "Participant ID not in signing set".to_string(),
        ));
    }
    if participant_id == 0 {
        return Err(BitcoinError::InvalidAddress(
            "Participant IDs must be 1-indexed (0 is reserved for the secret)".to_string(),
        ));
    }

    let xi = Fp::from_u64(participant_id as u64);
    let mut numerator = Fp::ONE;
    let mut denominator = Fp::ONE;

    for &other in participant_ids {
        if other == participant_id {
            continue;
        }
        if other == 0 {
            return Err(BitcoinError::InvalidAddress(
                "Participant IDs must be 1-indexed".to_string(),
            ));
        }
        let xj = Fp::from_u64(other as u64);
        // numerator *= x_j ; denominator *= (x_j - x_i)
        numerator = numerator.mul(&xj);
        let diff = xj.sub(&xi);
        if diff.is_zero() {
            return Err(BitcoinError::InvalidAddress(
                "Duplicate participant IDs in signing set".to_string(),
            ));
        }
        denominator = denominator.mul(&diff);
    }

    let inv_denominator = denominator.invert().ok_or_else(|| {
        BitcoinError::InvalidAddress("Non-invertible Lagrange denominator".to_string())
    })?;
    let lambda = numerator.mul(&inv_denominator);
    lambda.to_scalar_allow_zero()
}

/// Verify a Shamir share against Feldman VSS commitments:
/// `s_i·G == Σ_j i^j · C_j`.
fn feldman_verify(
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    participant_id: usize,
    share: &SecretKey,
    commitments: &[PublicKey],
) -> Result<bool, BitcoinError> {
    let x = Fp::from_u64(participant_id as u64);

    // Right-hand side: Σ_j i^j · C_j, evaluated with Horner over the points.
    // RHS = C_0 + x·(C_1 + x·(C_2 + … )).
    let mut acc: Option<PublicKey> = None;
    for c_j in commitments.iter().rev() {
        acc = Some(match acc {
            None => *c_j,
            Some(prev) => {
                let scaled = scalar_mul_point(secp, &prev, &x)?.ok_or_else(|| {
                    BitcoinError::InvalidAddress(
                        "Feldman accumulator reached identity unexpectedly".to_string(),
                    )
                })?;
                scaled.combine(c_j).map_err(|e| {
                    BitcoinError::InvalidAddress(format!("Feldman combine failed: {e}"))
                })?
            }
        });
    }
    let rhs = acc.ok_or_else(|| {
        BitcoinError::InvalidAddress("No Feldman commitments to verify against".to_string())
    })?;

    // Left-hand side: s_i·G.
    let lhs = PublicKey::from_secret_key(secp, share);
    Ok(lhs == rhs)
}

/// Multiply a point by a field scalar, returning `None` for the identity
/// (`scalar == 0`).
fn scalar_mul_point(
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    point: &PublicKey,
    scalar: &Fp,
) -> Result<Option<PublicKey>, BitcoinError> {
    if scalar.is_zero() {
        return Ok(None);
    }
    let s = scalar.to_scalar()?;
    let p = point
        .mul_tweak(secp, &s)
        .map_err(|e| BitcoinError::InvalidAddress(format!("Point scalar mul failed: {e}")))?;
    Ok(Some(p))
}

// ---------------------------------------------------------------------------
// GF(n) scalar field arithmetic (n = secp256k1 group order).
//
// `bitcoin::secp256k1::Scalar` is validation-only (no arithmetic) and
// `SecretKey` arithmetic rejects the zero scalar and offers no modular inverse,
// so threshold operations (Lagrange interpolation, partial-signature sums that
// may hit zero, signed differences) require an explicit field implementation.
// `Fp` is a 256-bit unsigned integer stored as four little-endian 64-bit limbs,
// always kept reduced modulo `n`.
// ---------------------------------------------------------------------------

/// secp256k1 group order as little-endian limbs.
const N_LIMBS: [u64; 4] = [
    0xBFD2_5E8C_D036_4141,
    0xBAAE_DCE6_AF48_A03B,
    0xFFFF_FFFF_FFFF_FFFE,
    0xFFFF_FFFF_FFFF_FFFF,
];

/// An element of `GF(n)` where `n` is the secp256k1 group order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fp {
    /// Little-endian 64-bit limbs, always reduced modulo `n`.
    limbs: [u64; 4],
}

impl Fp {
    const ZERO: Fp = Fp { limbs: [0; 4] };
    const ONE: Fp = Fp {
        limbs: [1, 0, 0, 0],
    };

    fn from_u64(value: u64) -> Fp {
        Fp {
            limbs: [value, 0, 0, 0],
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs == [0u64; 4]
    }

    /// Construct from a big-endian 32-byte value, reducing modulo `n`.
    fn from_be_bytes(bytes: &[u8; 32]) -> Result<Fp, BitcoinError> {
        let mut limbs = [0u64; 4];
        for (i, limb) in limbs.iter_mut().enumerate() {
            let start = 32 - (i + 1) * 8;
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(&bytes[start..start + 8]);
            *limb = u64::from_be_bytes(chunk);
        }
        Ok(Fp { limbs }.reduce_once())
    }

    fn from_be_slice(bytes: &[u8]) -> Result<Fp, BitcoinError> {
        if bytes.len() != 32 {
            return Err(BitcoinError::InvalidAddress(
                "Expected 32-byte scalar".to_string(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Fp::from_be_bytes(&arr)
    }

    fn from_secret_key(sk: &SecretKey) -> Fp {
        // SecretKey bytes are always a valid (< n) scalar.
        Fp::from_be_slice(&sk.secret_bytes()).unwrap_or(Fp::ZERO)
    }

    fn from_scalar(scalar: &Scalar) -> Fp {
        Fp::from_be_bytes(&scalar.to_be_bytes()).unwrap_or(Fp::ZERO)
    }

    fn to_be_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, limb) in self.limbs.iter().enumerate() {
            let start = 32 - (i + 1) * 8;
            out[start..start + 8].copy_from_slice(&limb.to_be_bytes());
        }
        out
    }

    /// Convert to a `SecretKey`; errors on the zero scalar (invalid secret key).
    fn to_secret_key(self) -> Result<SecretKey, BitcoinError> {
        SecretKey::from_slice(&self.to_be_bytes()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Field element is not a valid secret key: {e}"))
        })
    }

    /// Convert to a `Scalar` (used as an EC tweak); errors on the zero scalar,
    /// which is not a valid multiplicative tweak.
    fn to_scalar(self) -> Result<Scalar, BitcoinError> {
        Scalar::from_be_bytes(self.to_be_bytes())
            .map_err(|_| BitcoinError::InvalidAddress("Scalar out of range".to_string()))
    }

    /// Convert to a `Scalar`, mapping the zero scalar to `Scalar::ZERO` rather
    /// than erroring (used for additive challenge/binding factors).
    fn to_scalar_allow_zero(self) -> Result<Scalar, BitcoinError> {
        if self.is_zero() {
            return Ok(Scalar::ZERO);
        }
        self.to_scalar()
    }

    /// Reduce a value known to be `< 2n` by conditionally subtracting `n`.
    fn reduce_once(self) -> Fp {
        if cmp_limbs(&self.limbs, &N_LIMBS) != std::cmp::Ordering::Less {
            Fp {
                limbs: sub_limbs(&self.limbs, &N_LIMBS).0,
            }
        } else {
            self
        }
    }

    fn add(&self, other: &Fp) -> Fp {
        let (sum, carry) = add_limbs(&self.limbs, &other.limbs);
        if carry == 1 {
            // 2^256 + sum ≡ sum + (2^256 mod n). Equivalent: subtract n once
            // accounting for the carry. Since both inputs are < n, sum < 2n, so
            // the overflowed value minus n fits in 256 bits.
            let (reduced, _) = sub_limbs(&sum, &N_LIMBS);
            Fp { limbs: reduced }
        } else {
            Fp { limbs: sum }.reduce_once()
        }
    }

    fn negate(&self) -> Fp {
        if self.is_zero() {
            return Fp::ZERO;
        }
        Fp {
            limbs: sub_limbs(&N_LIMBS, &self.limbs).0,
        }
    }

    fn sub(&self, other: &Fp) -> Fp {
        self.add(&other.negate())
    }

    fn mul(&self, other: &Fp) -> Fp {
        let product = mul_limbs_wide(&self.limbs, &other.limbs);
        barrett_like_reduce(&product)
    }

    /// Modular inverse via Fermat's little theorem: `a^(n-2) mod n`.
    /// Returns `None` for the zero element.
    fn invert(&self) -> Option<Fp> {
        if self.is_zero() {
            return None;
        }
        // exponent = n - 2.
        let mut exp = N_LIMBS;
        // Subtract 2 (n is odd, low limb >= 2, so no borrow propagation issues).
        let (low, borrow) = exp[0].overflowing_sub(2);
        exp[0] = low;
        if borrow {
            // Extremely unlikely for secp256k1's n, handled for completeness.
            let mut i = 1;
            let mut b = true;
            while b && i < 4 {
                let (v, bb) = exp[i].overflowing_sub(1);
                exp[i] = v;
                b = bb;
                i += 1;
            }
        }
        Some(self.pow(&exp))
    }

    /// Square-and-multiply exponentiation with a little-endian limb exponent.
    fn pow(&self, exp: &[u64; 4]) -> Fp {
        let mut result = Fp::ONE;
        let mut base = *self;
        for limb in exp.iter() {
            let mut bits = *limb;
            for _ in 0..64 {
                if bits & 1 == 1 {
                    result = result.mul(&base);
                }
                base = base.mul(&base);
                bits >>= 1;
            }
        }
        result
    }

    /// Evaluate a polynomial (coefficients low-to-high) at `x` via Horner.
    fn eval_poly(coeffs: &[Fp], x: &Fp) -> Fp {
        let mut acc = Fp::ZERO;
        for c in coeffs.iter().rev() {
            acc = acc.mul(x).add(c);
        }
        acc
    }
}

/// Compare two little-endian limb arrays.
fn cmp_limbs(a: &[u64; 4], b: &[u64; 4]) -> std::cmp::Ordering {
    for i in (0..4).rev() {
        match a[i].cmp(&b[i]) {
            std::cmp::Ordering::Equal => continue,
            non_eq => return non_eq,
        }
    }
    std::cmp::Ordering::Equal
}

/// Add two little-endian limb arrays, returning the sum and the final carry.
fn add_limbs(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = [0u64; 4];
    let mut carry: u128 = 0;
    for i in 0..4 {
        let s = a[i] as u128 + b[i] as u128 + carry;
        out[i] = s as u64;
        carry = s >> 64;
    }
    (out, carry as u64)
}

/// Subtract `b` from `a` (little-endian), returning the difference and the
/// final borrow (1 if `a < b`).
fn sub_limbs(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = [0u64; 4];
    let mut borrow: i128 = 0;
    for i in 0..4 {
        let d = a[i] as i128 - b[i] as i128 - borrow;
        if d < 0 {
            out[i] = (d + (1i128 << 64)) as u64;
            borrow = 1;
        } else {
            out[i] = d as u64;
            borrow = 0;
        }
    }
    (out, borrow as u64)
}

/// Full 256x256 -> 512-bit schoolbook multiplication (eight little-endian limbs).
fn mul_limbs_wide(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
    let mut out = [0u64; 8];
    for i in 0..4 {
        let mut carry: u128 = 0;
        for j in 0..4 {
            let cur = out[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry;
            out[i + j] = cur as u64;
            carry = cur >> 64;
        }
        out[i + 4] = out[i + 4].wrapping_add(carry as u64);
    }
    out
}

/// Reduce a 512-bit value modulo `n` using long division by repeated
/// conditional subtraction of shifted `n`. Deterministic and exact.
fn barrett_like_reduce(wide: &[u64; 8]) -> Fp {
    // Work in a bit-by-bit long-division style: process the 512-bit number from
    // the most-significant bit down, maintaining a 256-bit remainder.
    let mut remainder = [0u64; 4];

    for bit in (0..512).rev() {
        // remainder <<= 1
        let mut carry = 0u64;
        for limb in remainder.iter_mut() {
            let new_carry = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = new_carry;
        }
        // The top bit shifted out of the 256-bit remainder is discarded only
        // after we fold in the next dividend bit and reduce; track it so we can
        // still subtract n when remainder >= 2^256.
        let overflow = carry;

        // Bring down the next bit of the dividend.
        let word = bit / 64;
        let off = bit % 64;
        let next_bit = (wide[word] >> off) & 1;
        remainder[0] |= next_bit;

        // If remainder (possibly with the overflow bit) >= n, subtract n.
        if overflow == 1 || cmp_limbs(&remainder, &N_LIMBS) != std::cmp::Ordering::Less {
            let (sub, _) = sub_limbs(&remainder, &N_LIMBS);
            remainder = sub;
        }
    }

    Fp { limbs: remainder }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::rand::rngs::OsRng;

    #[test]
    fn test_config_creation() {
        let config = FrostConfig::new(2, 3).unwrap();
        assert_eq!(config.threshold, 2);
        assert_eq!(config.total_participants, 3);
        assert!(config.is_valid());
    }

    #[test]
    fn test_config_validation() {
        assert!(FrostConfig::new(0, 3).is_err());
        assert!(FrostConfig::new(4, 3).is_err());
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

    // ---- GF(n) field arithmetic sanity checks --------------------------------

    #[test]
    fn test_field_add_sub_roundtrip() {
        let a = Fp::from_u64(12345);
        let b = Fp::from_u64(67890);
        let c = a.add(&b);
        assert_eq!(c.sub(&b), a);
        assert_eq!(c.sub(&a), b);
    }

    #[test]
    fn test_field_negate() {
        let a = Fp::from_u64(42);
        let neg = a.negate();
        assert!(a.add(&neg).is_zero());
        assert_eq!(Fp::ZERO.negate(), Fp::ZERO);
    }

    #[test]
    fn test_field_mul_identity_and_inverse() {
        let a = Fp::from_u64(7);
        assert_eq!(a.mul(&Fp::ONE), a);
        let inv = a.invert().unwrap();
        assert_eq!(a.mul(&inv), Fp::ONE);
        assert!(Fp::ZERO.invert().is_none());
    }

    #[test]
    fn test_field_inverse_random() {
        for _ in 0..20 {
            let sk = SecretKey::new(&mut OsRng);
            let a = Fp::from_secret_key(&sk);
            let inv = a.invert().unwrap();
            assert_eq!(a.mul(&inv), Fp::ONE, "a * a^-1 must be 1");
        }
    }

    #[test]
    fn test_field_mul_matches_secret_key_tweak() {
        // Cross-check Fp::mul against libsecp256k1 SecretKey::mul_tweak for
        // non-zero operands (where mul_tweak is defined).
        for _ in 0..20 {
            let a_sk = SecretKey::new(&mut OsRng);
            let b_sk = SecretKey::new(&mut OsRng);
            let a = Fp::from_secret_key(&a_sk);
            let b = Fp::from_secret_key(&b_sk);
            let product = a.mul(&b);
            if product.is_zero() {
                continue;
            }
            let expected = a_sk
                .mul_tweak(&Scalar::from_be_bytes(b_sk.secret_bytes()).unwrap())
                .unwrap();
            assert_eq!(product.to_be_bytes(), expected.secret_bytes());
        }
    }

    #[test]
    fn test_field_add_matches_secret_key_tweak() {
        for _ in 0..20 {
            let a_sk = SecretKey::new(&mut OsRng);
            let b_sk = SecretKey::new(&mut OsRng);
            let a = Fp::from_secret_key(&a_sk);
            let b = Fp::from_secret_key(&b_sk);
            let sum = a.add(&b);
            if sum.is_zero() {
                continue;
            }
            let expected = a_sk
                .add_tweak(&Scalar::from_be_bytes(b_sk.secret_bytes()).unwrap())
                .unwrap();
            assert_eq!(sum.to_be_bytes(), expected.secret_bytes());
        }
    }

    // ---- Shamir / Lagrange reconstruction ------------------------------------

    #[test]
    fn test_lagrange_reconstructs_secret() {
        // f(x) = secret + a1 x with a 2-of-n setup; check Σ λ_i f(i) = secret.
        let secret = Fp::from_u64(0x1234_5678);
        let a1 = Fp::from_u64(0x9abc_def0);
        let coeffs = [secret, a1];

        let ids = [2usize, 5usize];
        let mut reconstructed = Fp::ZERO;
        for &id in &ids {
            let s_i = Fp::eval_poly(&coeffs, &Fp::from_u64(id as u64));
            let lambda = Fp::from_scalar(&lagrange_coefficient(id, &ids).unwrap());
            reconstructed = reconstructed.add(&lambda.mul(&s_i));
        }
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_lagrange_rejects_outside_set() {
        assert!(lagrange_coefficient(4, &[1, 2, 3]).is_err());
    }

    // ---- Key generation + Feldman VSS ----------------------------------------

    #[test]
    fn test_keygen() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();

        let keygen_output = coordinator.keygen().unwrap();

        assert_eq!(keygen_output.shares.len(), 3);
        assert_eq!(keygen_output.verification_keys.len(), 3);
        assert_eq!(keygen_output.commitments.len(), 2); // threshold coefficients
        assert!(coordinator.group_pubkey().is_some());
    }

    #[test]
    fn test_keygen_group_key_is_even_y() {
        let config = FrostConfig::new(3, 5).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen = coordinator.keygen().unwrap();
        // commitments[0] is the full group key; must be even-Y for BIP-340.
        let (_, parity) = keygen.commitments[0].x_only_public_key();
        assert_eq!(parity, bitcoin::secp256k1::Parity::Even);
        // And its x-only form equals the advertised group pubkey.
        let (xonly, _) = keygen.commitments[0].x_only_public_key();
        assert_eq!(xonly, keygen.group_pubkey);
    }

    #[test]
    fn test_feldman_verify_each_share() {
        let config = FrostConfig::new(3, 5).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen = coordinator.keygen().unwrap();

        for share in &keygen.shares {
            let ok = coordinator
                .verify_share(share, &keygen.commitments)
                .unwrap();
            assert!(ok, "share {} must verify against Feldman commitments", share.participant_id);
        }
    }

    #[test]
    fn test_feldman_rejects_tampered_share() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen = coordinator.keygen().unwrap();

        // Tamper a share by replacing it with a different valid secret key.
        let mut bad = keygen.shares[0].clone();
        let other = SecretKey::new(&mut OsRng);
        bad.share = other;
        let ok = coordinator.verify_share(&bad, &keygen.commitments).unwrap();
        assert!(!ok, "a tampered share must NOT verify");
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

    // ---- End-to-end threshold signing ----------------------------------------

    /// Run a full t-of-n signing session with the chosen signer ids and return
    /// the aggregated signature, group key, and message.
    fn run_signing_session(
        threshold: usize,
        total: usize,
        signer_indices: &[usize],
    ) -> Result<([u8; 64], XOnlyPublicKey, [u8; 32]), BitcoinError> {
        let config = FrostConfig::new(threshold, total)?;
        let mut coordinator = FrostCoordinator::new(config)?;
        let keygen = coordinator.keygen()?;
        let group_pubkey = keygen.group_pubkey;

        // Build signers for the selected participants.
        let mut signers: Vec<FrostSigner> = signer_indices
            .iter()
            .map(|&idx| FrostSigner::new(keygen.shares[idx].clone()))
            .collect::<Result<_, _>>()?;

        // Round 1: nonce commitments.
        let mut commitments = Vec::new();
        for s in signers.iter_mut() {
            commitments.push(s.generate_nonces()?);
        }

        // Round 2: partial signatures.
        let message = [0x42u8; 32];
        let mut shares = Vec::new();
        for s in &signers {
            shares.push(s.sign(&message, &commitments, &group_pubkey)?);
        }

        let sig = coordinator.aggregate_signatures(&message, &commitments, &shares)?;
        Ok((sig, group_pubkey, message))
    }

    #[test]
    fn test_sign_2_of_3_verifies() {
        let secp = Secp256k1::new();
        // Try several signer subsets to exercise different Lagrange sets.
        for subset in [[0usize, 1usize], [0, 2], [1, 2]] {
            let (sig, group_pubkey, message) = run_signing_session(2, 3, &subset).unwrap();
            let signature = schnorr::Signature::from_slice(&sig).unwrap();
            let msg = Message::from_digest(message);
            secp.verify_schnorr(&signature, &msg, &group_pubkey)
                .unwrap_or_else(|e| panic!("2-of-3 subset {subset:?} must verify: {e}"));
            // The R component (first 32 bytes) must never be all zeros.
            assert_ne!(&sig[..32], &[0u8; 32], "R must be a real point, not a placeholder");
        }
    }

    #[test]
    fn test_sign_3_of_5_verifies() {
        let secp = Secp256k1::new();
        let (sig, group_pubkey, message) = run_signing_session(3, 5, &[0, 2, 4]).unwrap();
        let signature = schnorr::Signature::from_slice(&sig).unwrap();
        let msg = Message::from_digest(message);
        secp.verify_schnorr(&signature, &msg, &group_pubkey).unwrap();
    }

    #[test]
    fn test_sign_1_of_1_verifies() {
        let secp = Secp256k1::new();
        let (sig, group_pubkey, message) = run_signing_session(1, 1, &[0]).unwrap();
        let signature = schnorr::Signature::from_slice(&sig).unwrap();
        let msg = Message::from_digest(message);
        secp.verify_schnorr(&signature, &msg, &group_pubkey).unwrap();
    }

    #[test]
    fn test_repeated_signing_sessions_verify() {
        // Fresh nonces and even/odd-Y R values across runs must all verify.
        let secp = Secp256k1::new();
        for _ in 0..10 {
            let (sig, group_pubkey, message) = run_signing_session(2, 3, &[0, 1]).unwrap();
            let signature = schnorr::Signature::from_slice(&sig).unwrap();
            let msg = Message::from_digest(message);
            secp.verify_schnorr(&signature, &msg, &group_pubkey).unwrap();
        }
    }

    #[test]
    fn test_t_minus_one_cannot_sign() {
        // With only t-1 = 1 partial signature, aggregation must refuse, and the
        // wrong-Lagrange "signature" must NOT verify even if forced through.
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen = coordinator.keygen().unwrap();
        let group_pubkey = keygen.group_pubkey;

        let mut signer1 = FrostSigner::new(keygen.shares[0].clone()).unwrap();
        let commitments = vec![signer1.generate_nonces().unwrap()];
        let message = [0x07u8; 32];
        let share1 = signer1.sign(&message, &commitments, &group_pubkey).unwrap();

        // Coordinator enforces the threshold: fewer than t shares is rejected.
        let result =
            coordinator.aggregate_signatures(&message, &commitments, std::slice::from_ref(&share1));
        assert!(result.is_err(), "t-1 signers must not aggregate a signature");

        // Even constructing a raw (R.x, z) from a single share fails to verify,
        // because a single uncombined share is not a valid group signature.
        let secp = Secp256k1::new();
        let (group_nonce, _) =
            compute_group_nonce(&secp, &commitments, &group_pubkey, &message).unwrap();
        let (r_xonly, _) = group_nonce.x_only_public_key();
        let mut forged = [0u8; 64];
        forged[..32].copy_from_slice(&r_xonly.serialize());
        forged[32..].copy_from_slice(&share1.share);
        if let Ok(sig) = schnorr::Signature::from_slice(&forged) {
            let msg = Message::from_digest(message);
            assert!(
                secp.verify_schnorr(&sig, &msg, &group_pubkey).is_err(),
                "a single partial share must not verify as a group signature"
            );
        }
    }

    #[test]
    fn test_tampered_partial_signature_detected() {
        let config = FrostConfig::new(2, 3).unwrap();
        let mut coordinator = FrostCoordinator::new(config).unwrap();
        let keygen = coordinator.keygen().unwrap();
        let group_pubkey = keygen.group_pubkey;

        let mut signer1 = FrostSigner::new(keygen.shares[0].clone()).unwrap();
        let mut signer2 = FrostSigner::new(keygen.shares[1].clone()).unwrap();
        let c1 = signer1.generate_nonces().unwrap();
        let c2 = signer2.generate_nonces().unwrap();
        let commitments = vec![c1, c2];

        let message = [0x99u8; 32];
        let share1 = signer1.sign(&message, &commitments, &group_pubkey).unwrap();
        let mut share2 = signer2.sign(&message, &commitments, &group_pubkey).unwrap();

        // Tamper with share2: flip a byte.
        share2.share[0] ^= 0xFF;

        // Per-share verification must flag the tampered share...
        let secp = Secp256k1::new();
        let good = verify_partial_signature(
            &secp,
            &share1,
            &commitments,
            &keygen.shares[0].verification_key,
            &group_pubkey,
            &message,
        )
        .unwrap();
        assert!(good, "untampered share must verify");

        let bad = verify_partial_signature(
            &secp,
            &share2,
            &commitments,
            &keygen.shares[1].verification_key,
            &group_pubkey,
            &message,
        )
        .unwrap();
        assert!(!bad, "tampered share must be detected by per-share verification");

        // ...and aggregation must reject the resulting signature.
        let agg = coordinator.aggregate_signatures(&message, &commitments, &[share1, share2]);
        assert!(agg.is_err(), "aggregation with a tampered share must fail");
    }

    #[test]
    fn test_aggregate_requires_threshold() {
        let config = FrostConfig::new(3, 5).unwrap();
        let coordinator = FrostCoordinator::new(config).unwrap();
        let message = [0u8; 32];
        let err = coordinator.aggregate_signatures(&message, &[], &[]);
        assert!(err.is_err());
    }
}
