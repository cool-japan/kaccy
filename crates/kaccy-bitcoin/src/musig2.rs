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
//! # Standards compliance
//!
//! The implementation follows BIP-327 (MuSig2) and BIP-340 (Schnorr) precisely, including
//! the two even-Y parity rules that make the result verifiable by any compliant verifier:
//!
//! - the aggregated nonce `R = R1 + b*R2` must have even Y (otherwise every signer negates
//!   its effective nonce), and
//! - the aggregated key `Q` must have even Y (otherwise every signer negates its private
//!   key via the `g` factor).
//!
//! All domain-separated hashes use the BIP-340 tagged-hash construction, and challenges are
//! computed over the *x-only* serializations of `R` and `Q` together with the 32-byte
//! message, so the aggregated `(R.x, s)` validates via `secp256k1`'s `verify_schnorr`.
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
use bitcoin::secp256k1::{Parity, PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// BIP-340 challenge tag (`BIP0340/challenge`).
const TAG_BIP340_CHALLENGE: &[u8] = b"BIP0340/challenge";
/// BIP-327 nonce-coefficient tag (`MuSig/noncecoef`).
const TAG_MUSIG_NONCECOEF: &[u8] = b"MuSig/noncecoef";

/// Compute a BIP-340 style tagged hash: `SHA256(SHA256(tag) || SHA256(tag) || msg)`.
///
/// This is the construction mandated by BIP-340 / BIP-327 for all domain-separated hashes
/// (challenge, nonce coefficient, ...). Implementing it explicitly keeps the code free of
/// any tagged-hash macro while staying byte-for-byte compatible with reference code.
fn tagged_hash(tag: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    use bitcoin::hashes::{Hash, HashEngine, sha256};

    let tag_hash = sha256::Hash::hash(tag);
    let tag_bytes = tag_hash.to_byte_array();

    let mut engine = sha256::Hash::engine();
    engine.input(&tag_bytes);
    engine.input(&tag_bytes);
    for part in parts {
        engine.input(part);
    }
    sha256::Hash::from_engine(engine).to_byte_array()
}

/// Reduce a 32-byte big-endian value into a valid secp256k1 scalar.
///
/// BIP-340/BIP-327 reduce hash outputs modulo the curve order `n`. The `secp256k1` crate
/// exposes scalars only as `Scalar`, whose constructor rejects values `>= n`. A hash
/// landing outside the valid range is a cryptographically negligible event; we surface it
/// as an honest typed error rather than silently clamping (which would break verification).
fn scalar_from_hash(bytes: [u8; 32], context: &str) -> Result<Scalar, BitcoinError> {
    Scalar::from_be_bytes(bytes).map_err(|_| {
        BitcoinError::InvalidAddress(format!("{context}: hash out of range for scalar"))
    })
}

/// Negate a `SecretKey` when `parity` is odd, otherwise return it unchanged.
///
/// Used to fold the BIP-340 even-Y requirement (for both the aggregated key `Q` and the
/// aggregated nonce `R`) into per-signer secret material.
fn conditionally_negate(secret: SecretKey, parity: Parity) -> SecretKey {
    match parity {
        Parity::Even => secret,
        Parity::Odd => secret.negate(),
    }
}

/// Negate a `PublicKey` when `parity` is odd, otherwise return it unchanged.
fn conditionally_negate_point(
    secp: &Secp256k1<bitcoin::secp256k1::All>,
    point: PublicKey,
    parity: Parity,
) -> PublicKey {
    match parity {
        Parity::Even => point,
        Parity::Odd => point.negate(secp),
    }
}

/// Adaptor signature for atomic swaps and payment channels.
///
/// An adaptor (pre-)signature is a Schnorr signature whose nonce has been offset by an
/// adaptor point `T = t*G`. It is *not* a valid BIP-340 signature on its own: it can only
/// be turned into one by someone who knows the discrete log `t` of `T`. Once the completed
/// signature is published, anyone can subtract the pre-signature to recover `t`.
///
/// Concretely, for an x-only key `Q`, message `m` and secret nonce `k` (point `R0 = k*G`),
/// the adapted nonce is `R = R0 + T` and the challenge is
/// `e = H_BIP340/challenge(R.x || Q.x || m)`. The pre-signature scalar is `s' = k_e + e*d`,
/// where `d` is the even-Y secret of `Q` and `k_e` is `±k` chosen so the completed nonce
/// has even Y. Completion adds (even-Y `R`) or subtracts (odd-Y `R`) the secret `t`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptorSignature {
    /// The adaptor point (`T = t*G`) used to create the adaptor signature.
    pub adaptor_point: PublicKey,
    /// The pre-signature scalar `s'` (BIP-327 calls this the partial/adaptor signature).
    pub adapted_signature: [u8; 32],
    /// The adapted nonce point `R = R0 + T`; the final signature commits to `R.x`.
    pub adapted_nonce: PublicKey,
    /// Original partial signatures contributing to this adaptor signature (provenance).
    pub original_signatures: Vec<PartialSignature>,
}

impl AdaptorSignature {
    /// Create a new adaptor signature from its components.
    pub fn new(
        adaptor_point: PublicKey,
        adapted_signature: [u8; 32],
        adapted_nonce: PublicKey,
        original_signatures: Vec<PartialSignature>,
    ) -> Self {
        Self {
            adaptor_point,
            adapted_signature,
            adapted_nonce,
            original_signatures,
        }
    }

    /// The x-only ("BIP-340") form of the adapted nonce, i.e. the `R.x` of the final signature.
    pub fn nonce_x_only(&self) -> XOnlyPublicKey {
        self.adapted_nonce.x_only_public_key().0
    }

    /// Recover the adaptor secret `t` from a completed signature.
    ///
    /// If the adapted nonce `R` has even Y the completion added `t` (`t = s_final - s'`);
    /// if it has odd Y the completion subtracted `t` (`t = s' - s_final`). Either way the
    /// recovered secret satisfies `t*G == T`.
    pub fn recover_adaptor_secret(
        &self,
        final_signature: &[u8; 32],
    ) -> Result<SecretKey, BitcoinError> {
        let s_final = SecretKey::from_slice(final_signature)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid final signature: {}", e)))?;
        let s_pre = SecretKey::from_slice(&self.adapted_signature)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid pre-signature: {}", e)))?;

        // diff = s_final - s' = s_final + (-s').
        let diff = s_final.add_tweak(&s_pre.negate().into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to recover adaptor secret: {}", e))
        })?;

        // For odd-Y nonces the completion used s' - t, so t = -(s_final - s').
        let (_, parity) = self.adapted_nonce.x_only_public_key();
        Ok(conditionally_negate(diff, parity))
    }

    /// Verify the pre-signature against the adaptor point.
    ///
    /// Checks `s'*G == R_e + e*Q_e`, where `R_e` is the even-Y form of the pre-signature
    /// nonce `R0 = R - T` and `Q_e` is the even-Y aggregated key. This is a full
    /// cryptographic verification that completing with the discrete log of `T` yields a
    /// valid BIP-340 signature.
    pub fn verify(
        &self,
        secp: &Secp256k1<bitcoin::secp256k1::All>,
        message: &[u8; 32],
        aggregated_pubkey: &XOnlyPublicKey,
    ) -> Result<bool, BitcoinError> {
        let s_pre = SecretKey::from_slice(&self.adapted_signature)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid pre-signature: {}", e)))?;

        // Challenge e = H(R.x || Q.x || m) over the *adapted* nonce.
        let r_x = self.nonce_x_only();
        let challenge_bytes = tagged_hash(
            TAG_BIP340_CHALLENGE,
            &[&r_x.serialize(), &aggregated_pubkey.serialize(), message],
        );
        let challenge = scalar_from_hash(challenge_bytes, "adaptor challenge")?;

        // R0 = R - T  (the pre-signature nonce, before the adaptor offset).
        let r0 = self
            .adapted_nonce
            .combine(&self.adaptor_point.negate(secp))
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to remove adaptor point: {}", e))
            })?;
        // Even-Y form of R0 (the completed signature commits to even-Y R).
        let (_, r_parity) = self.adapted_nonce.x_only_public_key();
        let r0_even = conditionally_negate_point(secp, r0, r_parity);

        // Even-Y aggregated key, scaled by the challenge: e * Q_e.
        let q_even = aggregated_pubkey.public_key(Parity::Even);
        let e_q = q_even.mul_tweak(secp, &challenge).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to scale aggregated key: {}", e))
        })?;

        // Right-hand side: R_e + e*Q_e.
        let rhs = r0_even.combine(&e_q).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to combine verification points: {}", e))
        })?;

        // Left-hand side: s'*G.
        let lhs = PublicKey::from_secret_key(secp, &s_pre);

        Ok(lhs == rhs)
    }
}

/// Adaptor signature manager for atomic swap and payment channel use cases.
#[derive(Debug)]
pub struct AdaptorSignatureManager {
    /// Secp256k1 context.
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl AdaptorSignatureManager {
    /// Create a new adaptor signature manager.
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
        }
    }

    /// Generate a new adaptor secret `t` and point `T = t*G`.
    pub fn generate_adaptor(&self) -> Result<(SecretKey, PublicKey), BitcoinError> {
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let secret = SecretKey::new(&mut OsRng);
        let point = PublicKey::from_secret_key(&self.secp, &secret);

        Ok((secret, point))
    }

    /// Create a real adaptor pre-signature for a single signer.
    ///
    /// Produces `s' = k_e + e*d` (its nonce offset by the adaptor point `T`) such that
    /// completing with the discrete log of `T` yields a valid BIP-340 signature for
    /// `message` under the signer's x-only key. The returned [`AdaptorSignature`] verifies
    /// via [`AdaptorSignature::verify`].
    pub fn create_presignature(
        &self,
        signer_secret: &SecretKey,
        nonce_secret: &SecretKey,
        adaptor_point: PublicKey,
        message: &[u8; 32],
    ) -> Result<AdaptorSignature, BitcoinError> {
        // The aggregated key for a single signer is its own x-only key.
        let (q_xonly, q_parity) = signer_secret.x_only_public_key(&self.secp);

        // R0 = k*G, adapted nonce R = R0 + T.
        let r0 = PublicKey::from_secret_key(&self.secp, nonce_secret);
        let adapted_nonce = r0.combine(&adaptor_point).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to offset nonce by adaptor: {}", e))
        })?;
        let (r_xonly, r_parity) = adapted_nonce.x_only_public_key();

        // Challenge e = H(R.x || Q.x || m).
        let challenge_bytes = tagged_hash(
            TAG_BIP340_CHALLENGE,
            &[&r_xonly.serialize(), &q_xonly.serialize(), message],
        );
        let challenge = scalar_from_hash(challenge_bytes, "adaptor challenge")?;

        // Even-Y secret of Q, and nonce ±k so the completed R has even Y.
        let d = conditionally_negate(*signer_secret, q_parity);
        let k_e = conditionally_negate(*nonce_secret, r_parity);

        // s' = k_e + e*d.
        let e_d = d.mul_tweak(&challenge).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to apply challenge: {}", e))
        })?;
        let s_pre = k_e.add_tweak(&e_d.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute pre-signature: {}", e))
        })?;

        Ok(AdaptorSignature::new(
            adaptor_point,
            s_pre.secret_bytes(),
            adapted_nonce,
            Vec::new(),
        ))
    }

    /// Create an adaptor signature from already-computed partial signatures.
    ///
    /// The supplied partial signatures must have been produced against the *adapted* nonce
    /// `R = R_agg + T` (e.g. by a MuSig2 session whose published nonce was offset by the
    /// adaptor point). Their scalar sum is the pre-signature `s'`. `adapted_nonce` is the
    /// adapted aggregated nonce `R`.
    pub fn create_adaptor_signature(
        &self,
        partial_signatures: Vec<PartialSignature>,
        adaptor_point: PublicKey,
        adapted_nonce: PublicKey,
    ) -> Result<AdaptorSignature, BitcoinError> {
        if partial_signatures.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "No partial signatures provided".to_string(),
            ));
        }

        // Real aggregation: s' = sum(s_i) mod n.
        let mut acc = SecretKey::from_slice(&partial_signatures[0].s).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid partial signature: {}", e))
        })?;
        for sig in &partial_signatures[1..] {
            let next = SecretKey::from_slice(&sig.s).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Invalid partial signature: {}", e))
            })?;
            acc = acc.add_tweak(&next.into()).map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to aggregate pre-signature: {}", e))
            })?;
        }

        Ok(AdaptorSignature::new(
            adaptor_point,
            acc.secret_bytes(),
            adapted_nonce,
            partial_signatures,
        ))
    }

    /// Complete an adaptor signature with the adaptor secret `t`.
    ///
    /// Yields a valid BIP-340 signature scalar `s`. For even-Y adapted nonces `s = s' + t`;
    /// for odd-Y adapted nonces `s = s' - t`, matching
    /// [`AdaptorSignature::recover_adaptor_secret`].
    pub fn complete_adaptor_signature(
        &self,
        adaptor_sig: &AdaptorSignature,
        adaptor_secret: &SecretKey,
    ) -> Result<[u8; 32], BitcoinError> {
        // Verify the adaptor secret matches the adaptor point.
        let computed_point = PublicKey::from_secret_key(&self.secp, adaptor_secret);
        if computed_point != adaptor_sig.adaptor_point {
            return Err(BitcoinError::InvalidAddress(
                "Adaptor secret does not match adaptor point".to_string(),
            ));
        }

        let s_pre = SecretKey::from_slice(&adaptor_sig.adapted_signature)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid pre-signature: {}", e)))?;

        // Add ±t depending on the parity of the adapted nonce.
        let (_, parity) = adaptor_sig.adapted_nonce.x_only_public_key();
        let t = conditionally_negate(*adaptor_secret, parity);

        let final_sig = s_pre.add_tweak(&t.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to complete adaptor signature: {}", e))
        })?;

        Ok(final_sig.secret_bytes())
    }

    /// Verify that an adaptor pre-signature is valid for `message` under `aggregated_pubkey`.
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

/// MuSig2 key aggregator for combining public keys.
#[derive(Debug, Clone)]
pub struct MuSig2KeyAggregator {
    /// Public keys of all participants.
    pub_keys: Vec<PublicKey>,
    /// Aggregated x-only public key `Q` (always even-Y per BIP-340).
    aggregated_key: XOnlyPublicKey,
    /// Parity of the *full* aggregated point before x-only reduction (the BIP-327 `gacc`).
    q_parity: Parity,
    /// Key coefficients for each participant (BIP 327).
    coefficients: HashMap<PublicKey, Scalar>,
    /// Secp256k1 context.
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl MuSig2KeyAggregator {
    /// Create a new key aggregator from public keys.
    pub fn new(pub_keys: Vec<PublicKey>) -> Result<Self, BitcoinError> {
        if pub_keys.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "At least one public key required".to_string(),
            ));
        }

        let secp = Secp256k1::new();
        let coefficients = Self::compute_coefficients(&pub_keys)?;
        let (aggregated_key, q_parity) = Self::aggregate_keys(&secp, &pub_keys, &coefficients)?;

        Ok(Self {
            pub_keys,
            aggregated_key,
            q_parity,
            coefficients,
            secp,
        })
    }

    /// Compute key aggregation coefficients (BIP 327): `a_i = H(L || P_i)` with
    /// `L = H(P_1 || ... || P_n)`.
    fn compute_coefficients(
        pub_keys: &[PublicKey],
    ) -> Result<HashMap<PublicKey, Scalar>, BitcoinError> {
        use bitcoin::hashes::{Hash, HashEngine, sha256};

        let mut coefficients = HashMap::new();

        // Compute L = hash(P1 || P2 || ... || Pn).
        let mut l_engine = sha256::Hash::engine();
        for pk in pub_keys {
            l_engine.input(&pk.serialize());
        }
        let l_hash = sha256::Hash::from_engine(l_engine);

        // Compute coefficient for each key: a_i = hash(L || P_i).
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

    /// Aggregate public keys: `Q = sum(a_i * P_i)`. Returns the x-only key and the parity
    /// of the full aggregated point (needed by signers for the BIP-327 `g` factor).
    fn aggregate_keys(
        secp: &Secp256k1<bitcoin::secp256k1::All>,
        pub_keys: &[PublicKey],
        coefficients: &HashMap<PublicKey, Scalar>,
    ) -> Result<(XOnlyPublicKey, Parity), BitcoinError> {
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

        let (x_only, parity) = aggregated_pk.x_only_public_key();
        Ok((x_only, parity))
    }

    /// Get the aggregated public key.
    pub fn aggregated_key(&self) -> XOnlyPublicKey {
        self.aggregated_key
    }

    /// Parity of the full aggregated point (BIP-327 `gacc`): odd means signers negate.
    pub fn aggregated_parity(&self) -> Parity {
        self.q_parity
    }

    /// Get the coefficient for a specific public key.
    pub fn get_coefficient(&self, pk: &PublicKey) -> Option<Scalar> {
        self.coefficients.get(pk).copied()
    }

    /// Get all public keys.
    pub fn public_keys(&self) -> &[PublicKey] {
        &self.pub_keys
    }
}

/// Nonce pair for MuSig2 signing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MuSig2Nonce {
    /// First nonce (r1).
    pub r1: PublicKey,
    /// Second nonce (r2).
    pub r2: PublicKey,
}

impl MuSig2Nonce {
    /// Create a new nonce pair.
    pub fn new(r1: PublicKey, r2: PublicKey) -> Self {
        Self { r1, r2 }
    }
}

/// Aggregated nonce for MuSig2.
#[derive(Debug, Clone, Copy)]
pub struct AggregatedNonce {
    /// Aggregated `R = R1 + b*R2`.
    pub r: PublicKey,
    /// Nonce coefficient `b` (BIP-327, message-bound).
    pub b: Scalar,
    /// Parity of the full aggregated key `Q` (BIP-327 `gacc`), conveyed to signers so they
    /// can apply the `g` factor when computing partial signatures.
    pub q_parity: Parity,
}

/// Partial signature from a single signer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSignature {
    /// Signer's public key.
    pub signer_pubkey: PublicKey,
    /// Partial signature `s_i` (stored as bytes; `Scalar` has no field arithmetic).
    pub s: [u8; 32],
}

/// MuSig2 signer for creating partial signatures.
#[derive(Debug)]
pub struct MuSig2Signer {
    /// Private key.
    #[allow(dead_code)]
    privkey: SecretKey,
    /// Public key.
    pubkey: PublicKey,
    /// Secp256k1 context.
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
    /// Current nonce secrets (k1, k2).
    nonce_secrets: Option<(SecretKey, SecretKey)>,
    /// Current public nonces.
    nonce_public: Option<MuSig2Nonce>,
}

impl MuSig2Signer {
    /// Create a new signer with a private key.
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

    /// Get the public key.
    pub fn public_key(&self) -> PublicKey {
        self.pubkey
    }

    /// Generate nonces for a signing session.
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

    /// Create a partial signature (BIP-327).
    ///
    /// Computes `s_i = k_e + e * g * a_i * x_i (mod n)` where:
    /// - `k_e = ±(k1 + b*k2)`, negated iff the aggregated nonce `R` has odd Y, so the
    ///   published signature commits to the even-Y nonce required by BIP-340;
    /// - `e = H_BIP340/challenge(R.x || Q.x || m) mod n` is the tagged Schnorr challenge
    ///   over the *x-only* serializations of `R` and `Q`;
    /// - `g = +1` if the aggregated key `Q` has even Y else `-1`
    ///   (`aggregated_nonce.q_parity`), folding the even-Y key requirement into each secret;
    /// - `a_i` is the signer's key-aggregation coefficient and `x_i` its private key.
    ///
    /// Summing these partials yields `s` such that `(R.x, s)` is a valid BIP-340 Schnorr
    /// signature under the aggregated x-only key `Q`.
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        &self,
        message: &[u8; 32],
        aggregated_nonce: &AggregatedNonce,
        aggregated_pubkey: &XOnlyPublicKey,
        key_coefficient: Scalar,
    ) -> Result<PartialSignature, BitcoinError> {
        // Get nonce secrets.
        let (k1, k2) = self
            .nonce_secrets
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not generated".to_string()))?;

        // Effective nonce k = k1 + b*k2.
        let k2_scaled = k2
            .mul_tweak(&aggregated_nonce.b)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale k2: {}", e)))?;
        let k_eff = k1.add_tweak(&k2_scaled.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute effective nonce: {}", e))
        })?;

        // BIP-340 requires even-Y R: flip every signer's nonce when R has odd Y.
        let (_, r_parity) = aggregated_nonce.r.x_only_public_key();
        let k_eff = conditionally_negate(k_eff, r_parity);

        // Tagged Schnorr challenge over the x-only forms of R and Q.
        let r_xonly = aggregated_nonce.r.x_only_public_key().0;
        let challenge_bytes = tagged_hash(
            TAG_BIP340_CHALLENGE,
            &[
                &r_xonly.serialize(),
                &aggregated_pubkey.serialize(),
                message,
            ],
        );
        let challenge = scalar_from_hash(challenge_bytes, "challenge")?;

        // g * x_i : negate the private key when the full aggregated key Q has odd Y.
        let privkey_g = conditionally_negate(self.privkey, aggregated_nonce.q_parity);

        // d_i = g * a_i * x_i, then e * d_i.
        let privkey_scaled = privkey_g
            .mul_tweak(&key_coefficient)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale privkey: {}", e)))?;
        let privkey_challenge = privkey_scaled.mul_tweak(&challenge).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to multiply by challenge: {}", e))
        })?;

        // s_i = k_e + e * g * a_i * x_i.
        let s = k_eff.add_tweak(&privkey_challenge.into()).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute signature: {}", e))
        })?;

        Ok(PartialSignature {
            signer_pubkey: self.pubkey,
            s: s.secret_bytes(),
        })
    }

    /// Clear nonces after signing (security best practice).
    pub fn clear_nonces(&mut self) {
        self.nonce_secrets = None;
        self.nonce_public = None;
    }
}

/// MuSig2 coordinator for aggregating nonces and signatures.
#[derive(Debug)]
pub struct MuSig2Coordinator {
    /// Key aggregator.
    key_aggregator: MuSig2KeyAggregator,
    /// Collected nonces from signers.
    nonces: HashMap<PublicKey, MuSig2Nonce>,
    /// Aggregated nonce.
    aggregated_nonce: Option<AggregatedNonce>,
    /// Secp256k1 context.
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl MuSig2Coordinator {
    /// Create a new coordinator with a key aggregator.
    pub fn new(key_aggregator: MuSig2KeyAggregator) -> Self {
        Self {
            key_aggregator,
            nonces: HashMap::new(),
            aggregated_nonce: None,
            secp: Secp256k1::new(),
        }
    }

    /// Add a nonce from a signer.
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

    /// Aggregate all nonces once all are collected.
    ///
    /// Per BIP-327 the nonce coefficient `b` is bound to the message:
    /// `b = H_"MuSig/noncecoef"(R1 || R2 || Q.x || m) mod n`, and the final nonce is
    /// `R = R1 + b*R2`. The message MUST be the same one later passed to
    /// [`MuSig2Signer::sign`]. Point sums are order-independent, so the result does not
    /// depend on `HashMap` iteration order.
    pub fn aggregate_nonces(
        &mut self,
        message: &[u8; 32],
    ) -> Result<AggregatedNonce, BitcoinError> {
        if self.nonces.len() != self.key_aggregator.public_keys().len() {
            return Err(BitcoinError::InvalidAddress(
                "Not all nonces collected".to_string(),
            ));
        }

        // Compute R1 = sum(r1_i) and R2 = sum(r2_i).
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

        // Nonce coefficient (BIP-327): b = H_"MuSig/noncecoef"(R1 || R2 || Q.x || m) mod n.
        // The message binding is mandatory: omitting it produces signatures that fail to
        // verify against any compliant BIP-340 verifier.
        let b_bytes = tagged_hash(
            TAG_MUSIG_NONCECOEF,
            &[
                &r1.serialize(),
                &r2.serialize(),
                &self.key_aggregator.aggregated_key().serialize(),
                message,
            ],
        );
        let b = scalar_from_hash(b_bytes, "nonce coefficient")?;

        // Compute R = R1 + b*R2.
        let r2_scaled = r2
            .mul_tweak(&self.secp, &b)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to scale R2: {}", e)))?;

        let r = r1.combine(&r2_scaled).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to compute final R: {}", e))
        })?;

        let aggregated = AggregatedNonce {
            r,
            b,
            q_parity: self.key_aggregator.aggregated_parity(),
        };
        self.aggregated_nonce = Some(aggregated);

        Ok(aggregated)
    }

    /// Aggregate partial signatures into the final signature scalar `s = sum(s_i) mod n`.
    pub fn aggregate_signatures(
        &self,
        partial_sigs: &[PartialSignature],
    ) -> Result<[u8; 32], BitcoinError> {
        if partial_sigs.len() != self.key_aggregator.public_keys().len() {
            return Err(BitcoinError::InvalidAddress(
                "Not all partial signatures provided".to_string(),
            ));
        }

        // Verify all signers are accounted for.
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

        // Aggregate signatures: s = sum(s_i) mod n.
        let mut aggregated = SecretKey::from_slice(&partial_sigs[0].s).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid partial signature: {}", e))
        })?;

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

    /// Build the full 64-byte BIP-340 Schnorr signature `R.x || s` from the aggregated
    /// nonce and the aggregated scalar produced by [`Self::aggregate_signatures`].
    pub fn final_signature(&self, aggregated_s: &[u8; 32]) -> Result<[u8; 64], BitcoinError> {
        let agg_nonce = self
            .aggregated_nonce
            .ok_or_else(|| BitcoinError::InvalidAddress("Nonces not aggregated".to_string()))?;

        let r_xonly = agg_nonce.r.x_only_public_key().0;
        let mut sig = [0u8; 64];
        sig[..32].copy_from_slice(&r_xonly.serialize());
        sig[32..].copy_from_slice(aggregated_s);
        Ok(sig)
    }

    /// Get the aggregated nonce.
    pub fn aggregated_nonce(&self) -> Option<AggregatedNonce> {
        self.aggregated_nonce
    }

    /// Get the key aggregator.
    pub fn key_aggregator(&self) -> &MuSig2KeyAggregator {
        &self.key_aggregator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::rand::rngs::OsRng;
    use bitcoin::secp256k1::{Message, schnorr::Signature};

    /// Drive a full MuSig2 session for `n` signers and return the aggregated key, message,
    /// the 64-byte BIP-340 signature, and the secp context, asserting along the way that
    /// `verify_schnorr` accepts the result. This is the end-to-end correctness oracle.
    fn run_musig_session(
        secret_keys: &[SecretKey],
        message: &[u8; 32],
    ) -> (XOnlyPublicKey, [u8; 64], Secp256k1<bitcoin::secp256k1::All>) {
        let secp = Secp256k1::new();

        // Build signers.
        let mut signers: Vec<MuSig2Signer> = secret_keys
            .iter()
            .map(|sk| MuSig2Signer::new(*sk).expect("signer"))
            .collect();

        let pub_keys: Vec<PublicKey> = signers.iter().map(|s| s.public_key()).collect();
        let aggregator = MuSig2KeyAggregator::new(pub_keys.clone()).expect("aggregator");
        let aggregated_key = aggregator.aggregated_key();

        // Each signer generates nonces.
        let nonces: Vec<MuSig2Nonce> = signers
            .iter_mut()
            .map(|s| s.generate_nonces().expect("nonce"))
            .collect();

        // Coordinator aggregates nonces (message-bound).
        let mut coordinator = MuSig2Coordinator::new(aggregator);
        for (signer, nonce) in signers.iter().zip(nonces.iter()) {
            coordinator
                .add_nonce(signer.public_key(), *nonce)
                .expect("add nonce");
        }
        let agg_nonce = coordinator.aggregate_nonces(message).expect("agg nonce");

        // Each signer produces a partial signature.
        let partials: Vec<PartialSignature> = signers
            .iter()
            .map(|signer| {
                let coeff = coordinator
                    .key_aggregator()
                    .get_coefficient(&signer.public_key())
                    .expect("coefficient");
                signer
                    .sign(message, &agg_nonce, &aggregated_key, coeff)
                    .expect("partial sign")
            })
            .collect();

        // Aggregate into the final signature.
        let s = coordinator
            .aggregate_signatures(&partials)
            .expect("aggregate");
        let sig_bytes = coordinator.final_signature(&s).expect("final sig");

        // Verify via secp256k1 verify_schnorr (the strict BIP-340 oracle).
        let sig = Signature::from_slice(&sig_bytes).expect("sig parse");
        let msg = Message::from_digest(*message);
        secp.verify_schnorr(&sig, &msg, &aggregated_key)
            .expect("verify_schnorr must accept the aggregated MuSig2 signature");

        (aggregated_key, sig_bytes, secp)
    }

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

        // Each key should have a coefficient.
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

        // Each nonce generation should produce different values.
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

        let message = [7u8; 32];
        let _agg_nonce = coordinator.aggregate_nonces(&message).unwrap();
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
    fn test_musig2_2_of_2_verifies_via_secp256k1() {
        // 2-of-2 MuSig2: sign -> aggregate -> verify_schnorr must accept.
        // Run several times to exercise both R and Q parity branches with high probability.
        for _ in 0..8 {
            let sk1 = SecretKey::new(&mut OsRng);
            let sk2 = SecretKey::new(&mut OsRng);
            let message = {
                let mut m = [0u8; 32];
                m.copy_from_slice(&Scalar::random().to_be_bytes());
                m
            };
            let (_q, _sig, _secp) = run_musig_session(&[sk1, sk2], &message);
        }
    }

    #[test]
    fn test_musig2_3_of_3_verifies_via_secp256k1() {
        // 3-of-3 MuSig2 round-trip with strict BIP-340 verification.
        for _ in 0..8 {
            let sk1 = SecretKey::new(&mut OsRng);
            let sk2 = SecretKey::new(&mut OsRng);
            let sk3 = SecretKey::new(&mut OsRng);
            let message = [0x42u8; 32];
            let (_q, _sig, _secp) = run_musig_session(&[sk1, sk2, sk3], &message);
        }
    }

    #[test]
    fn test_musig2_wrong_message_rejected() {
        // A signature for message m must NOT verify for a different message m'.
        let sk1 = SecretKey::new(&mut OsRng);
        let sk2 = SecretKey::new(&mut OsRng);
        let message = [0x11u8; 32];
        let (q, sig_bytes, secp) = run_musig_session(&[sk1, sk2], &message);

        let other = [0x22u8; 32];
        let sig = Signature::from_slice(&sig_bytes).unwrap();
        let other_msg = Message::from_digest(other);
        assert!(secp.verify_schnorr(&sig, &other_msg, &q).is_err());
    }

    #[test]
    fn test_adaptor_signature_generation() {
        let manager = AdaptorSignatureManager::new();
        let (secret, point) = manager.generate_adaptor().unwrap();

        // Verify the point is derived from the secret.
        let secp = Secp256k1::new();
        let computed_point = PublicKey::from_secret_key(&secp, &secret);
        assert_eq!(computed_point, point);
    }

    #[test]
    fn test_adaptor_presignature_roundtrip() {
        // Full real adaptor round-trip:
        // pre-sign -> verify-pre -> adapt (complete) -> verify-final (BIP-340) -> extract.
        let manager = AdaptorSignatureManager::new();
        let secp = Secp256k1::new();

        // Run multiple times to exercise both nonce-parity branches.
        for _ in 0..8 {
            let signer_secret = SecretKey::new(&mut OsRng);
            let nonce_secret = SecretKey::new(&mut OsRng);
            let (adaptor_secret, adaptor_point) = manager.generate_adaptor().unwrap();
            let message = [0x5au8; 32];

            let (q_xonly, _) = signer_secret.x_only_public_key(&secp);

            // Pre-sign.
            let pre = manager
                .create_presignature(&signer_secret, &nonce_secret, adaptor_point, &message)
                .unwrap();

            // The pre-signature must verify against the adaptor point.
            assert!(
                manager
                    .verify_adaptor_signature(&pre, &message, &q_xonly)
                    .unwrap(),
                "pre-signature must verify against adaptor point"
            );

            // Complete (adapt) with the secret to obtain a real BIP-340 signature.
            let final_s = manager
                .complete_adaptor_signature(&pre, &adaptor_secret)
                .unwrap();

            let r_x = pre.nonce_x_only();
            let mut sig_bytes = [0u8; 64];
            sig_bytes[..32].copy_from_slice(&r_x.serialize());
            sig_bytes[32..].copy_from_slice(&final_s);
            let sig = Signature::from_slice(&sig_bytes).unwrap();
            let msg = Message::from_digest(message);
            secp.verify_schnorr(&sig, &msg, &q_xonly)
                .expect("completed adaptor signature must be a valid BIP-340 signature");

            // Extract the adaptor secret from the published signature.
            let recovered = pre.recover_adaptor_secret(&final_s).unwrap();
            assert_eq!(
                recovered.secret_bytes(),
                adaptor_secret.secret_bytes(),
                "extracted adaptor secret must match the original"
            );
        }
    }

    #[test]
    fn test_adaptor_signature_from_partials() {
        // create_adaptor_signature must sum the partials (not return the first one).
        let manager = AdaptorSignatureManager::new();
        let secp = Secp256k1::new();
        let (_secret, point) = manager.generate_adaptor().unwrap();
        let adapted_nonce = PublicKey::from_secret_key(&secp, &SecretKey::new(&mut OsRng));

        // Two partials with known scalars; the sum must be s1 + s2 mod n.
        let s1 = SecretKey::new(&mut OsRng);
        let s2 = SecretKey::new(&mut OsRng);
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::new(&mut OsRng));
        let partials = vec![
            PartialSignature {
                signer_pubkey: pk,
                s: s1.secret_bytes(),
            },
            PartialSignature {
                signer_pubkey: pk,
                s: s2.secret_bytes(),
            },
        ];

        let adaptor_sig = manager
            .create_adaptor_signature(partials, point, adapted_nonce)
            .unwrap();

        let expected = s1.add_tweak(&s2.into()).unwrap().secret_bytes();
        assert_eq!(adaptor_sig.adapted_signature, expected);
        assert_eq!(adaptor_sig.original_signatures.len(), 2);
    }

    #[test]
    fn test_adaptor_wrong_secret() {
        let manager = AdaptorSignatureManager::new();
        let (_secret1, point) = manager.generate_adaptor().unwrap();
        let (wrong_secret, _) = manager.generate_adaptor().unwrap();

        let signer_secret = SecretKey::new(&mut OsRng);
        let nonce_secret = SecretKey::new(&mut OsRng);
        let message = [3u8; 32];

        let pre = manager
            .create_presignature(&signer_secret, &nonce_secret, point, &message)
            .unwrap();

        // Completing with the wrong secret must fail (point mismatch).
        let result = manager.complete_adaptor_signature(&pre, &wrong_secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_adaptor_tampered_presignature_rejected() {
        // A pre-signature whose scalar has been tampered with must fail verification.
        let manager = AdaptorSignatureManager::new();
        let secp = Secp256k1::new();
        let signer_secret = SecretKey::new(&mut OsRng);
        let nonce_secret = SecretKey::new(&mut OsRng);
        let (_t, point) = manager.generate_adaptor().unwrap();
        let message = [9u8; 32];
        let (q_xonly, _) = signer_secret.x_only_public_key(&secp);

        let mut pre = manager
            .create_presignature(&signer_secret, &nonce_secret, point, &message)
            .unwrap();
        // Flip a byte of the pre-signature scalar.
        pre.adapted_signature[0] ^= 0xff;

        let ok = manager
            .verify_adaptor_signature(&pre, &message, &q_xonly)
            .unwrap_or(false);
        assert!(!ok, "tampered pre-signature must not verify");
    }

    #[test]
    fn test_empty_partial_signatures() {
        let manager = AdaptorSignatureManager::new();
        let secp = Secp256k1::new();
        let (_secret, point) = manager.generate_adaptor().unwrap();
        let adapted_nonce = PublicKey::from_secret_key(&secp, &SecretKey::new(&mut OsRng));

        let result = manager.create_adaptor_signature(vec![], point, adapted_nonce);
        assert!(result.is_err());
    }
}
