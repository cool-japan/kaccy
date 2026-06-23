//! Post-Quantum Cryptography
//!
//! This module provides real, pure-Rust post-quantum cryptographic primitives
//! built on the RustCrypto project, together with hybrid (classical + PQ)
//! constructions and migration tooling.
//!
//! Concretely:
//! - **Signatures** are implemented with ML-DSA-65 (FIPS 204, formerly
//!   CRYSTALS-Dilithium) via the [`ml_dsa`] crate.
//! - **Key encapsulation / encryption** is implemented with ML-KEM-768
//!   (FIPS 203, formerly CRYSTALS-Kyber) via the [`ml_kem`] crate.
//! - **Hybrid signatures** combine a classical secp256k1 ECDSA signature with a
//!   real ML-DSA-65 signature so deployments can migrate gradually while keeping
//!   AND-security where required.
//!
//! Algorithm families that do not (yet) have an audited pure-Rust RustCrypto
//! implementation in this workspace (code-based / multivariate) are represented
//! honestly: requesting them returns a typed [`PqcError`] rather than a fake key
//! or a no-op signature.

use anyhow::Result;
use ml_dsa::{
    EncodedSignature, EncodedVerifyingKey, Generate, MlDsa65, Seed as MlDsaSeed, Signature,
    SigningKey, VerifyingKey,
    signature::{Keypair, Signer, Verifier},
};
use ml_kem::{
    Ciphertext, Encapsulate, EncapsulationKey, Kem, MlKem768,
    kem::{Decapsulate, Key as KemKey, KeyExport as KemKeyExport},
};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey, ecdsa::Signature as EcdsaSignature};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Underlying ML-DSA parameter set used for every signature in this module.
///
/// `MlDsa65` (FIPS 204, NIST security category 3) is selected as it offers the
/// best balance of security and performance per the RustCrypto recommendation.
type SigParams = MlDsa65;

/// Underlying ML-KEM parameter set used for every key encapsulation in this
/// module.
///
/// `MlKem768` (FIPS 203, NIST security category 3) is selected to match the
/// signature security level.
type KemParams = MlKem768;

/// Errors that can arise from post-quantum cryptographic operations.
#[derive(Debug, Error)]
pub enum PqcError {
    /// The requested algorithm family has no real pure-Rust implementation
    /// wired into this module, so no honest key/signature can be produced.
    #[error(
        "post-quantum algorithm {0} is not implemented by a real pure-Rust backend in this build; \
         use a Lattice (ML-DSA) algorithm for signatures or the ML-KEM API for key exchange"
    )]
    UnsupportedAlgorithm(PQCAlgorithm),

    /// A stored key/signature/ciphertext byte string did not have the size or
    /// structure required by the underlying primitive.
    #[error("malformed {kind}: {detail}")]
    MalformedBytes {
        /// What kind of material failed to decode (e.g. "ML-DSA private key").
        kind: &'static str,
        /// Human-readable detail about the failure.
        detail: String,
    },

    /// A classical (secp256k1) cryptographic operation failed.
    #[error("classical secp256k1 operation failed: {0}")]
    Classical(#[from] secp256k1::Error),

    /// A required component of a hybrid construction was missing.
    #[error("hybrid signature is missing its {0} component")]
    MissingComponent(&'static str),
}

impl PqcError {
    fn malformed(kind: &'static str, detail: impl Into<String>) -> Self {
        Self::MalformedBytes {
            kind,
            detail: detail.into(),
        }
    }
}

/// Post-quantum cryptographic algorithm type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PQCAlgorithm {
    /// Lattice-based signatures. Backed by **ML-DSA-65** (FIPS 204).
    Lattice,
    /// Hash-based signatures (e.g., SPHINCS+).
    ///
    /// No audited pure-Rust SPHINCS+ backend is wired into this workspace, so
    /// for real signing/verification this variant is served by the same
    /// lattice primitive (ML-DSA-65) as [`PQCAlgorithm::Lattice`]. It remains a
    /// distinct policy/label used by the migration and threat-assessment tools.
    HashBased,
    /// Code-based (e.g., Classic McEliece). No real backend; honest error.
    CodeBased,
    /// Multivariate (e.g., Rainbow). No real backend; honest error.
    Multivariate,
}

impl fmt::Display for PQCAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PQCAlgorithm::Lattice => write!(f, "Lattice-based"),
            PQCAlgorithm::HashBased => write!(f, "Hash-based"),
            PQCAlgorithm::CodeBased => write!(f, "Code-based"),
            PQCAlgorithm::Multivariate => write!(f, "Multivariate"),
        }
    }
}

impl PQCAlgorithm {
    /// Whether this algorithm maps onto a real signature backend in this build.
    fn has_signature_backend(self) -> bool {
        matches!(self, PQCAlgorithm::Lattice | PQCAlgorithm::HashBased)
    }
}

/// Post-quantum **signature** key pair backed by ML-DSA-65 (FIPS 204).
///
/// The keys are real ML-DSA material:
/// - `private_key` holds the 32-byte ML-DSA seed (`ξ`), the preferred
///   serialization from which the expanded signing key is deterministically
///   reconstructed.
/// - `public_key` holds the encoded ML-DSA [`VerifyingKey`] (1952 bytes for
///   ML-DSA-65).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQKeyPair {
    /// Algorithm policy label used to generate this key pair.
    pub algorithm: PQCAlgorithm,
    /// Encoded ML-DSA verifying (public) key bytes.
    pub public_key: Vec<u8>,
    /// 32-byte ML-DSA seed from which the signing (private) key is derived.
    pub private_key: Vec<u8>,
    /// Total key size in bytes (`public_key.len() + private_key.len()`).
    pub key_size: usize,
}

impl PQKeyPair {
    /// Generate a real ML-DSA-65 signature key pair using the operating
    /// system's secure RNG.
    ///
    /// # Errors
    /// Returns [`PqcError::UnsupportedAlgorithm`] for algorithm families that
    /// have no real pure-Rust backend ([`PQCAlgorithm::CodeBased`],
    /// [`PQCAlgorithm::Multivariate`]).
    pub fn generate(algorithm: PQCAlgorithm) -> Result<Self> {
        if !algorithm.has_signature_backend() {
            return Err(PqcError::UnsupportedAlgorithm(algorithm).into());
        }

        // Real ML-DSA-65 key generation seeded from the OS CSPRNG.
        let signing_key = SigningKey::<SigParams>::generate();
        Self::from_signing_key(algorithm, &signing_key)
    }

    /// Deterministically generate a real ML-DSA-65 key pair from a 32-byte seed.
    ///
    /// This is primarily useful for known-answer tests and reproducible key
    /// derivation. The mapping seed → (signing key, verifying key) is exactly
    /// FIPS 204 `ML-DSA.KeyGen_internal`.
    ///
    /// # Errors
    /// Returns [`PqcError::UnsupportedAlgorithm`] for unsupported families and
    /// [`PqcError::MalformedBytes`] if `seed` is not exactly 32 bytes.
    pub fn generate_from_seed(algorithm: PQCAlgorithm, seed: &[u8]) -> Result<Self> {
        if !algorithm.has_signature_backend() {
            return Err(PqcError::UnsupportedAlgorithm(algorithm).into());
        }
        let seed = MlDsaSeed::try_from(seed).map_err(|_| {
            PqcError::malformed(
                "ML-DSA seed",
                format!("expected 32 bytes, got {}", seed.len()),
            )
        })?;
        let signing_key = SigningKey::<SigParams>::from_seed(&seed);
        Self::from_signing_key(algorithm, &signing_key)
    }

    fn from_signing_key(
        algorithm: PQCAlgorithm,
        signing_key: &SigningKey<SigParams>,
    ) -> Result<Self> {
        let private_key = signing_key.to_seed().to_vec();
        let public_key = signing_key.verifying_key().encode().to_vec();
        let key_size = public_key.len() + private_key.len();
        Ok(Self {
            algorithm,
            public_key,
            private_key,
            key_size,
        })
    }

    /// Reconstruct the ML-DSA signing key from the stored 32-byte seed.
    fn signing_key(&self) -> Result<SigningKey<SigParams>> {
        if !self.algorithm.has_signature_backend() {
            return Err(PqcError::UnsupportedAlgorithm(self.algorithm).into());
        }
        let seed = MlDsaSeed::try_from(self.private_key.as_slice()).map_err(|_| {
            PqcError::malformed(
                "ML-DSA private key",
                format!(
                    "expected a 32-byte seed, got {} bytes",
                    self.private_key.len()
                ),
            )
        })?;
        Ok(SigningKey::<SigParams>::from_seed(&seed))
    }

    /// Reconstruct the ML-DSA verifying key from the stored public-key bytes.
    fn verifying_key(&self) -> Result<VerifyingKey<SigParams>> {
        if !self.algorithm.has_signature_backend() {
            return Err(PqcError::UnsupportedAlgorithm(self.algorithm).into());
        }
        let encoded =
            EncodedVerifyingKey::<SigParams>::try_from(self.public_key.as_slice()).map_err(|_| {
                PqcError::malformed(
                    "ML-DSA public key",
                    format!(
                        "unexpected encoded verifying-key length {}",
                        self.public_key.len()
                    ),
                )
            })?;
        Ok(VerifyingKey::<SigParams>::decode(&encoded))
    }

    /// Produce a real ML-DSA-65 signature over `data`.
    ///
    /// Uses the deterministic (hedged with an all-zero randomizer) variant of
    /// ML-DSA with an empty context string, matching the RustCrypto
    /// [`Signer`] implementation.
    ///
    /// # Errors
    /// Returns a [`PqcError`] if the key material is malformed or the algorithm
    /// has no real backend.
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let signing_key = self.signing_key()?;
        let signature: Signature<SigParams> = signing_key
            .try_sign(data)
            .map_err(|e| PqcError::malformed("ML-DSA signing input", e.to_string()))?;
        Ok(signature.encode().to_vec())
    }

    /// Verify a real ML-DSA-65 signature over `data`.
    ///
    /// Returns `Ok(true)` only if `signature` is a valid ML-DSA-65 signature
    /// over `data` under this key pair's public key; returns `Ok(false)` for a
    /// well-formed-but-invalid signature (tampered message, tampered signature,
    /// or wrong key). Returns an `Err` only when the public key itself is
    /// malformed or the algorithm has no real backend.
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        let verifying_key = self.verifying_key()?;
        // A signature whose byte length does not match the parameter set can
        // never verify; treat it as an honest `false` rather than an error.
        let encoded = match EncodedSignature::<SigParams>::try_from(signature) {
            Ok(encoded) => encoded,
            Err(_) => return Ok(false),
        };
        let Some(signature) = Signature::<SigParams>::decode(&encoded) else {
            return Ok(false);
        };
        Ok(verifying_key.verify(data, &signature).is_ok())
    }
}

/// Post-quantum **key-encapsulation** key pair backed by ML-KEM-768 (FIPS 203).
///
/// ML-KEM lets anyone holding the [`encapsulation_key`](Self::encapsulation_key)
/// establish a shared secret with the holder of the decapsulation (private)
/// key. The bytes stored here are real ML-KEM material:
/// - `decapsulation_key` is the 64-byte ML-KEM seed (`d ‖ z`).
/// - `encapsulation_key` is the encoded ML-KEM encapsulation key (1184 bytes
///   for ML-KEM-768).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQKemKeyPair {
    /// Encoded ML-KEM encapsulation (public) key bytes.
    pub encapsulation_key: Vec<u8>,
    /// 64-byte ML-KEM seed from which the decapsulation (private) key derives.
    pub decapsulation_key: Vec<u8>,
}

/// Output of an ML-KEM encapsulation: the ciphertext to transmit plus the
/// shared secret to keep.
#[derive(Debug, Clone)]
pub struct PQEncapsulation {
    /// ML-KEM ciphertext (1088 bytes for ML-KEM-768) to send to the holder of
    /// the decapsulation key.
    pub ciphertext: Vec<u8>,
    /// The 32-byte shared secret derived by the encapsulator.
    pub shared_secret: Vec<u8>,
}

impl PQKemKeyPair {
    /// Generate a real ML-KEM-768 key pair using the OS secure RNG.
    pub fn generate() -> Self {
        let (decapsulation_key, encapsulation_key) = KemParams::generate_keypair();
        Self {
            encapsulation_key: encapsulation_key.to_bytes().to_vec(),
            // ML-KEM decapsulation keys generated this way always expose a seed.
            decapsulation_key: decapsulation_key
                .to_seed()
                .map(|seed| seed.to_vec())
                .unwrap_or_default(),
        }
    }

    /// Deterministically generate a real ML-KEM-768 key pair from a 64-byte
    /// seed. Useful for known-answer tests.
    ///
    /// # Errors
    /// Returns [`PqcError::MalformedBytes`] if `seed` is not exactly 64 bytes.
    pub fn generate_from_seed(seed: &[u8]) -> Result<Self> {
        let seed = ml_kem::Seed::try_from(seed).map_err(|_| {
            PqcError::malformed(
                "ML-KEM seed",
                format!("expected 64 bytes, got {}", seed.len()),
            )
        })?;
        let decapsulation_key = ml_kem::DecapsulationKey::<KemParams>::from_seed(seed);
        let encapsulation_key = decapsulation_key.encapsulation_key().to_bytes().to_vec();
        Ok(Self {
            encapsulation_key,
            decapsulation_key: decapsulation_key
                .to_seed()
                .map(|seed| seed.to_vec())
                .unwrap_or_default(),
        })
    }

    fn decode_encapsulation_key(&self) -> Result<EncapsulationKey<KemParams>> {
        let key = KemKey::<EncapsulationKey<KemParams>>::try_from(self.encapsulation_key.as_slice())
            .map_err(|_| {
                PqcError::malformed(
                    "ML-KEM encapsulation key",
                    format!(
                        "unexpected encoded length {}",
                        self.encapsulation_key.len()
                    ),
                )
            })?;
        EncapsulationKey::<KemParams>::new(&key).map_err(|_| {
            PqcError::malformed(
                "ML-KEM encapsulation key",
                "bytes are not a valid ML-KEM-768 encapsulation key",
            )
            .into()
        })
    }

    fn decode_decapsulation_key(&self) -> Result<ml_kem::DecapsulationKey<KemParams>> {
        let seed = ml_kem::Seed::try_from(self.decapsulation_key.as_slice()).map_err(|_| {
            PqcError::malformed(
                "ML-KEM decapsulation key",
                format!(
                    "expected a 64-byte seed, got {} bytes",
                    self.decapsulation_key.len()
                ),
            )
        })?;
        Ok(ml_kem::DecapsulationKey::<KemParams>::from_seed(seed))
    }

    /// Encapsulate a fresh shared secret to this key pair's encapsulation key
    /// using the OS secure RNG.
    ///
    /// # Errors
    /// Returns a [`PqcError`] if the stored encapsulation key is malformed.
    pub fn encapsulate(&self) -> Result<PQEncapsulation> {
        let encapsulation_key = self.decode_encapsulation_key()?;
        let (ciphertext, shared_secret) = encapsulation_key.encapsulate();
        Ok(PQEncapsulation {
            ciphertext: ciphertext.as_slice().to_vec(),
            shared_secret: shared_secret.as_slice().to_vec(),
        })
    }

    /// Decapsulate a ciphertext with this key pair's decapsulation key,
    /// recovering the shared secret chosen by the encapsulator.
    ///
    /// # Errors
    /// Returns a [`PqcError`] if the decapsulation key or ciphertext is
    /// malformed.
    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let decapsulation_key = self.decode_decapsulation_key()?;
        let ciphertext = Ciphertext::<KemParams>::try_from(ciphertext).map_err(|_| {
            PqcError::malformed(
                "ML-KEM ciphertext",
                format!("unexpected ciphertext length {}", ciphertext.len()),
            )
        })?;
        let shared_secret = decapsulation_key.decapsulate(&ciphertext);
        Ok(shared_secret.as_slice().to_vec())
    }
}

/// Hybrid cryptography combining a classical secp256k1 ECDSA signature with a
/// post-quantum ML-DSA-65 signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridCrypto {
    /// Classical secp256k1 secret key bytes (32 bytes).
    classical_private_key: Vec<u8>,
    /// Classical secp256k1 public key bytes (33 bytes, compressed SEC1).
    pub classical_public_key: Vec<u8>,
    /// Post-quantum signature key.
    pub pq_key: PQKeyPair,
    /// Hybrid mode.
    pub mode: HybridMode,
}

/// Hybrid mode for gradual migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HybridMode {
    /// Prefer classical, fallback to PQ. Produces a classical signature.
    ClassicalPrimary,
    /// Use both (AND security): a classical and a PQ signature are required.
    DualSignature,
    /// Prefer PQ, fallback to classical. Produces a PQ signature.
    PQPrimary,
    /// PQ only (full migration). Produces a PQ signature.
    PQOnly,
}

impl HybridCrypto {
    /// Create a new hybrid configuration with a fresh real secp256k1 key pair
    /// and a fresh real ML-DSA-65 key pair.
    ///
    /// # Errors
    /// Propagates [`PqcError`] if the requested PQ algorithm has no real
    /// backend.
    pub fn new(pq_algorithm: PQCAlgorithm, mode: HybridMode) -> Result<Self> {
        let secp = Secp256k1::new();
        // Use secp256k1's own bundled RNG so the `rand::Rng` trait bound is
        // satisfied by the exact rand version the crate was compiled against.
        let (secret_key, public_key) = secp.generate_keypair(&mut secp256k1::rand::rng());
        Ok(Self {
            classical_private_key: secret_key.secret_bytes().to_vec(),
            classical_public_key: public_key.serialize().to_vec(),
            pq_key: PQKeyPair::generate(pq_algorithm)?,
            mode,
        })
    }

    /// Hash `data` to a 32-byte secp256k1 message digest (SHA-256).
    fn classical_message(data: &[u8]) -> Message {
        let digest: [u8; 32] = Sha256::digest(data).into();
        Message::from_digest(digest)
    }

    /// Produce a real classical secp256k1 ECDSA signature over `data`.
    fn classical_sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let secret_key = SecretKey::from_byte_array(
            self.classical_private_key
                .as_slice()
                .try_into()
                .map_err(|_| {
                    PqcError::malformed(
                        "secp256k1 private key",
                        format!("expected 32 bytes, got {}", self.classical_private_key.len()),
                    )
                })?,
        )
        .map_err(PqcError::Classical)?;
        let secp = Secp256k1::signing_only();
        let signature = secp.sign_ecdsa(Self::classical_message(data), &secret_key);
        Ok(signature.serialize_compact().to_vec())
    }

    /// Verify a real classical secp256k1 ECDSA signature over `data`.
    fn classical_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        let public_key =
            PublicKey::from_slice(&self.classical_public_key).map_err(PqcError::Classical)?;
        let signature = match EcdsaSignature::from_compact(signature) {
            Ok(signature) => signature,
            Err(_) => return Ok(false),
        };
        let secp = Secp256k1::verification_only();
        Ok(secp
            .verify_ecdsa(Self::classical_message(data), &signature, &public_key)
            .is_ok())
    }

    /// Sign with the hybrid approach dictated by [`HybridCrypto::mode`].
    ///
    /// All produced signatures are real: the classical component is a genuine
    /// secp256k1 ECDSA signature and the PQ component is a genuine ML-DSA-65
    /// signature.
    pub fn hybrid_sign(&self, data: &[u8]) -> Result<HybridSignature> {
        match self.mode {
            HybridMode::ClassicalPrimary => Ok(HybridSignature {
                classical_sig: Some(self.classical_sign(data)?),
                pq_sig: None,
                mode: self.mode,
            }),
            HybridMode::PQPrimary | HybridMode::PQOnly => Ok(HybridSignature {
                classical_sig: None,
                pq_sig: Some(self.pq_key.sign(data)?),
                mode: self.mode,
            }),
            HybridMode::DualSignature => Ok(HybridSignature {
                classical_sig: Some(self.classical_sign(data)?),
                pq_sig: Some(self.pq_key.sign(data)?),
                mode: self.mode,
            }),
        }
    }

    /// Verify a hybrid signature against `data`.
    ///
    /// For [`HybridMode::DualSignature`] **both** the classical and the PQ
    /// signatures must be present and valid (AND security). For the other modes
    /// the single present component must be valid.
    ///
    /// # Errors
    /// Returns a [`PqcError`] if a required component is missing or key material
    /// is malformed.
    pub fn hybrid_verify(&self, data: &[u8], signature: &HybridSignature) -> Result<bool> {
        match self.mode {
            HybridMode::ClassicalPrimary => {
                let sig = signature
                    .classical_sig
                    .as_ref()
                    .ok_or(PqcError::MissingComponent("classical"))?;
                self.classical_verify(data, sig)
            }
            HybridMode::PQPrimary | HybridMode::PQOnly => {
                let sig = signature
                    .pq_sig
                    .as_ref()
                    .ok_or(PqcError::MissingComponent("post-quantum"))?;
                self.pq_key.verify(data, sig)
            }
            HybridMode::DualSignature => {
                let classical = signature
                    .classical_sig
                    .as_ref()
                    .ok_or(PqcError::MissingComponent("classical"))?;
                let pq = signature
                    .pq_sig
                    .as_ref()
                    .ok_or(PqcError::MissingComponent("post-quantum"))?;
                Ok(self.classical_verify(data, classical)? && self.pq_key.verify(data, pq)?)
            }
        }
    }
}

/// Hybrid signature containing both classical and PQ components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSignature {
    /// Classical (secp256k1 ECDSA) signature bytes, if present.
    pub classical_sig: Option<Vec<u8>>,
    /// Post-quantum (ML-DSA-65) signature bytes, if present.
    pub pq_sig: Option<Vec<u8>>,
    /// Hybrid mode that generated this signature.
    pub mode: HybridMode,
}

/// Migration plan for transitioning to PQC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQMigrationPlan {
    /// Current migration phase.
    pub current_phase: MigrationPhase,
    /// Target PQC algorithm to migrate to.
    pub target_algorithm: PQCAlgorithm,
    /// Total planned timeline in days.
    pub timeline_days: u32,
    /// Whether the migration maintains backward compatibility.
    pub backwards_compatible: bool,
}

/// Migration phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    /// Preparation and testing.
    Phase1Assessment,
    /// Deploy hybrid mode.
    Phase2HybridDeploy,
    /// Gradual rollout.
    Phase3GradualRollout,
    /// Full PQ deployment.
    Phase4FullPQ,
    /// Deprecate classical.
    Phase5DeprecateClassical,
}

impl PQMigrationPlan {
    /// Create a new migration plan starting at Phase 1 assessment.
    pub fn new(target_algorithm: PQCAlgorithm, timeline_days: u32) -> Self {
        Self {
            current_phase: MigrationPhase::Phase1Assessment,
            target_algorithm,
            timeline_days,
            backwards_compatible: true,
        }
    }

    /// Advance to the next migration phase and return it.
    pub fn advance_phase(&mut self) -> MigrationPhase {
        self.current_phase = match self.current_phase {
            MigrationPhase::Phase1Assessment => MigrationPhase::Phase2HybridDeploy,
            MigrationPhase::Phase2HybridDeploy => MigrationPhase::Phase3GradualRollout,
            MigrationPhase::Phase3GradualRollout => MigrationPhase::Phase4FullPQ,
            MigrationPhase::Phase4FullPQ => MigrationPhase::Phase5DeprecateClassical,
            MigrationPhase::Phase5DeprecateClassical => MigrationPhase::Phase5DeprecateClassical,
        };
        self.current_phase
    }

    /// Get the recommended hybrid mode for the current migration phase.
    pub fn get_recommended_hybrid_mode(&self) -> HybridMode {
        match self.current_phase {
            MigrationPhase::Phase1Assessment => HybridMode::ClassicalPrimary,
            MigrationPhase::Phase2HybridDeploy => HybridMode::DualSignature,
            MigrationPhase::Phase3GradualRollout => HybridMode::PQPrimary,
            MigrationPhase::Phase4FullPQ | MigrationPhase::Phase5DeprecateClassical => {
                HybridMode::PQOnly
            }
        }
    }
}

/// Quantum threat assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumThreatAssessment {
    /// Overall quantum threat level.
    pub threat_level: ThreatLevel,
    /// Estimated years until quantum computers can break current cryptography.
    pub estimated_years_until_threat: u8,
    /// Names of assets that need quantum-safe protection.
    pub critical_assets: Vec<String>,
    /// Recommended PQC algorithm based on threat assessment.
    pub recommended_algorithm: PQCAlgorithm,
}

/// Threat level from quantum computing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatLevel {
    /// Quantum threat is 10+ years away.
    Low,
    /// Quantum threat is 5–10 years away.
    Medium,
    /// Quantum threat is 2–5 years away.
    High,
    /// Quantum threat is less than 2 years away.
    Critical,
}

impl QuantumThreatAssessment {
    /// Build a threat assessment from estimated timeline and secret longevity.
    pub fn assess(years_until_threat: u8, has_long_term_secrets: bool) -> Self {
        let threat_level = match years_until_threat {
            0..=2 => ThreatLevel::Critical,
            3..=5 => ThreatLevel::High,
            6..=10 => ThreatLevel::Medium,
            _ => ThreatLevel::Low,
        };

        let recommended_algorithm = if has_long_term_secrets {
            // Prefer lattice-based (ML-DSA / ML-KEM) for long-term protection.
            PQCAlgorithm::Lattice
        } else {
            PQCAlgorithm::HashBased
        };

        Self {
            threat_level,
            estimated_years_until_threat: years_until_threat,
            critical_assets: Vec::new(),
            recommended_algorithm,
        }
    }

    /// Add an asset name to the list of critical assets.
    pub fn add_critical_asset(&mut self, asset: String) {
        self.critical_assets.push(asset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ML-DSA-65 signatures (FIPS 204) ----

    #[test]
    fn test_pq_keypair_generation_is_real() {
        let keypair = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen");
        assert_eq!(keypair.algorithm, PQCAlgorithm::Lattice);
        // ML-DSA-65: 32-byte seed private key, 1952-byte encoded verifying key.
        assert_eq!(keypair.private_key.len(), 32);
        assert_eq!(keypair.public_key.len(), 1952);
        assert_eq!(keypair.key_size, 32 + 1952);
        // Keys must NOT be zero-filled (the old fabrication produced all zeros).
        assert!(keypair.private_key.iter().any(|&b| b != 0));
        assert!(keypair.public_key.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_two_keypairs_differ() {
        let a = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen a");
        let b = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen b");
        assert_ne!(a.public_key, b.public_key);
        assert_ne!(a.private_key, b.private_key);
    }

    #[test]
    fn test_sign_then_verify_succeeds() {
        let keypair = PQKeyPair::generate(PQCAlgorithm::HashBased).expect("keygen");
        let data = b"test message";

        let signature = keypair.sign(data).expect("sign");
        // ML-DSA-65 signatures are 3309 bytes.
        assert_eq!(signature.len(), 3309);
        assert!(signature.iter().any(|&b| b != 0));

        assert!(keypair.verify(data, &signature).expect("verify"));
    }

    #[test]
    fn test_tampered_message_fails_verify() {
        let keypair = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen");
        let data = b"authentic message";
        let signature = keypair.sign(data).expect("sign");

        let tampered = b"authentic message!";
        assert!(!keypair.verify(tampered, &signature).expect("verify"));
    }

    #[test]
    fn test_tampered_signature_fails_verify() {
        let keypair = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen");
        let data = b"message to sign";
        let mut signature = keypair.sign(data).expect("sign");

        // Flip a bit somewhere in the middle of the signature.
        let mid = signature.len() / 2;
        signature[mid] ^= 0x01;
        assert!(!keypair.verify(data, &signature).expect("verify"));
    }

    #[test]
    fn test_empty_signature_fails_verify() {
        let keypair = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen");
        // The old fabrication accepted any non-empty signature; a real verify
        // must reject an empty (and any wrong-length) signature.
        assert!(!keypair.verify(b"data", &[]).expect("verify"));
        assert!(!keypair.verify(b"data", &[0u8; 16]).expect("verify"));
    }

    #[test]
    fn test_verify_under_wrong_public_key_fails() {
        let signer = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen signer");
        let attacker = PQKeyPair::generate(PQCAlgorithm::Lattice).expect("keygen attacker");
        let data = b"transfer 10 BTC";

        let signature = signer.sign(data).expect("sign");
        // Valid under the real signer, invalid under a different public key.
        assert!(signer.verify(data, &signature).expect("verify signer"));
        assert!(!attacker.verify(data, &signature).expect("verify attacker"));
    }

    #[test]
    fn test_deterministic_keygen_known_answer() {
        // Fixed seed → deterministic ML-DSA key pair (FIPS 204 KeyGen_internal).
        let seed = [7u8; 32];
        let a = PQKeyPair::generate_from_seed(PQCAlgorithm::Lattice, &seed).expect("keygen a");
        let b = PQKeyPair::generate_from_seed(PQCAlgorithm::Lattice, &seed).expect("keygen b");
        assert_eq!(a.public_key, b.public_key);
        assert_eq!(a.private_key, b.private_key);
        assert_eq!(a.private_key, seed.to_vec());

        // Deterministic signing → identical signatures, and they verify.
        let msg = b"known answer message";
        let sig_a = a.sign(msg).expect("sign a");
        let sig_b = b.sign(msg).expect("sign b");
        assert_eq!(sig_a, sig_b);
        assert!(a.verify(msg, &sig_a).expect("verify"));
    }

    #[test]
    fn test_bad_seed_length_is_typed_error() {
        let err = PQKeyPair::generate_from_seed(PQCAlgorithm::Lattice, &[0u8; 16]).unwrap_err();
        let pqc = err.downcast_ref::<PqcError>().expect("PqcError");
        assert!(matches!(pqc, PqcError::MalformedBytes { .. }));
    }

    #[test]
    fn test_unsupported_signature_algorithms_error_honestly() {
        for algorithm in [PQCAlgorithm::CodeBased, PQCAlgorithm::Multivariate] {
            let err = PQKeyPair::generate(algorithm).unwrap_err();
            let pqc = err.downcast_ref::<PqcError>().expect("PqcError");
            assert!(matches!(pqc, PqcError::UnsupportedAlgorithm(a) if *a == algorithm));
        }
    }

    // ---- ML-KEM-768 key encapsulation (FIPS 203) ----

    #[test]
    fn test_kem_keygen_is_real() {
        let kp = PQKemKeyPair::generate();
        // ML-KEM-768: 64-byte seed, 1184-byte encapsulation key.
        assert_eq!(kp.decapsulation_key.len(), 64);
        assert_eq!(kp.encapsulation_key.len(), 1184);
        assert!(kp.encapsulation_key.iter().any(|&b| b != 0));
        assert!(kp.decapsulation_key.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_kem_encapsulate_decapsulate_shared_secret_matches() {
        let kp = PQKemKeyPair::generate();
        let encapsulation = kp.encapsulate().expect("encapsulate");
        // ML-KEM-768: 1088-byte ciphertext, 32-byte shared secret.
        assert_eq!(encapsulation.ciphertext.len(), 1088);
        assert_eq!(encapsulation.shared_secret.len(), 32);

        let recovered = kp.decapsulate(&encapsulation.ciphertext).expect("decapsulate");
        assert_eq!(encapsulation.shared_secret, recovered);
    }

    #[test]
    fn test_kem_wrong_ciphertext_yields_different_secret() {
        // ML-KEM uses implicit rejection: a corrupted ciphertext decapsulates
        // to a pseudo-random secret that does NOT match the encapsulated one.
        let kp = PQKemKeyPair::generate();
        let encapsulation = kp.encapsulate().expect("encapsulate");
        let mut ciphertext = encapsulation.ciphertext.clone();
        ciphertext[0] ^= 0xff;
        let recovered = kp.decapsulate(&ciphertext).expect("decapsulate");
        assert_ne!(encapsulation.shared_secret, recovered);
    }

    #[test]
    fn test_kem_deterministic_keygen_known_answer() {
        let seed = [3u8; 64];
        let a = PQKemKeyPair::generate_from_seed(&seed).expect("keygen a");
        let b = PQKemKeyPair::generate_from_seed(&seed).expect("keygen b");
        assert_eq!(a.encapsulation_key, b.encapsulation_key);
        assert_eq!(a.decapsulation_key, b.decapsulation_key);

        // A secret encapsulated to `a` decapsulates correctly with `b` (same key).
        let encapsulation = a.encapsulate().expect("encapsulate");
        let recovered = b.decapsulate(&encapsulation.ciphertext).expect("decapsulate");
        assert_eq!(encapsulation.shared_secret, recovered);
    }

    #[test]
    fn test_kem_malformed_inputs_are_typed_errors() {
        let kp = PQKemKeyPair::generate();
        let err = kp.decapsulate(&[0u8; 10]).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<PqcError>(),
            Some(PqcError::MalformedBytes { .. })
        ));
        let err = PQKemKeyPair::generate_from_seed(&[0u8; 8]).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<PqcError>(),
            Some(PqcError::MalformedBytes { .. })
        ));
    }

    // ---- Hybrid (classical secp256k1 + ML-DSA) ----

    #[test]
    fn test_hybrid_crypto_dual_signature_roundtrip() {
        let hybrid = HybridCrypto::new(PQCAlgorithm::Lattice, HybridMode::DualSignature)
            .expect("hybrid setup");
        let data = b"important message";

        let sig = hybrid.hybrid_sign(data).expect("sign");
        assert!(sig.classical_sig.is_some());
        assert!(sig.pq_sig.is_some());
        // Both components are real signatures and the whole thing verifies.
        assert!(hybrid.hybrid_verify(data, &sig).expect("verify"));
        // Tampering breaks AND-security verification.
        assert!(!hybrid.hybrid_verify(b"tampered", &sig).expect("verify"));
    }

    #[test]
    fn test_hybrid_dual_requires_both_components() {
        let hybrid = HybridCrypto::new(PQCAlgorithm::Lattice, HybridMode::DualSignature)
            .expect("hybrid setup");
        let data = b"needs both";
        let mut sig = hybrid.hybrid_sign(data).expect("sign");
        // Drop the PQ component: verification must now fail (missing component).
        sig.pq_sig = None;
        assert!(hybrid.hybrid_verify(data, &sig).is_err());
    }

    #[test]
    fn test_hybrid_crypto_pq_only_roundtrip() {
        let hybrid =
            HybridCrypto::new(PQCAlgorithm::Lattice, HybridMode::PQOnly).expect("hybrid setup");
        let data = b"pq only message";

        let sig = hybrid.hybrid_sign(data).expect("sign");
        assert!(sig.classical_sig.is_none());
        assert!(sig.pq_sig.is_some());
        assert!(hybrid.hybrid_verify(data, &sig).expect("verify"));
    }

    #[test]
    fn test_hybrid_classical_primary_roundtrip() {
        let hybrid = HybridCrypto::new(PQCAlgorithm::Lattice, HybridMode::ClassicalPrimary)
            .expect("hybrid setup");
        let data = b"classical primary";
        let sig = hybrid.hybrid_sign(data).expect("sign");
        assert!(sig.classical_sig.is_some());
        assert!(sig.pq_sig.is_none());
        assert!(hybrid.hybrid_verify(data, &sig).expect("verify"));
        assert!(!hybrid.hybrid_verify(b"other", &sig).expect("verify"));
    }

    // ---- Migration / threat tooling (unchanged behaviour) ----

    #[test]
    fn test_migration_plan() {
        let mut plan = PQMigrationPlan::new(PQCAlgorithm::Lattice, 180);

        assert_eq!(plan.current_phase, MigrationPhase::Phase1Assessment);
        assert_eq!(
            plan.get_recommended_hybrid_mode(),
            HybridMode::ClassicalPrimary
        );

        plan.advance_phase();
        assert_eq!(plan.current_phase, MigrationPhase::Phase2HybridDeploy);
        assert_eq!(plan.get_recommended_hybrid_mode(), HybridMode::DualSignature);

        plan.advance_phase();
        assert_eq!(plan.current_phase, MigrationPhase::Phase3GradualRollout);

        plan.advance_phase();
        assert_eq!(plan.current_phase, MigrationPhase::Phase4FullPQ);
        assert_eq!(plan.get_recommended_hybrid_mode(), HybridMode::PQOnly);
    }

    #[test]
    fn test_quantum_threat_assessment() {
        let assessment = QuantumThreatAssessment::assess(3, true);
        assert_eq!(assessment.threat_level, ThreatLevel::High);
        assert_eq!(assessment.recommended_algorithm, PQCAlgorithm::Lattice);
    }

    #[test]
    fn test_threat_levels() {
        assert_eq!(
            QuantumThreatAssessment::assess(1, false).threat_level,
            ThreatLevel::Critical
        );
        assert_eq!(
            QuantumThreatAssessment::assess(7, false).threat_level,
            ThreatLevel::Medium
        );
        assert_eq!(
            QuantumThreatAssessment::assess(15, false).threat_level,
            ThreatLevel::Low
        );
    }

    #[test]
    fn test_critical_assets() {
        let mut assessment = QuantumThreatAssessment::assess(5, true);
        assessment.add_critical_asset("User private keys".to_string());
        assessment.add_critical_asset("Transaction signatures".to_string());

        assert_eq!(assessment.critical_assets.len(), 2);
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(format!("{}", PQCAlgorithm::Lattice), "Lattice-based");
        assert_eq!(format!("{}", PQCAlgorithm::HashBased), "Hash-based");
    }
}
