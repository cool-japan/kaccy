//! Reputation Portability for importing and exporting reputation data
//!
//! This module enables users to import reputation from external platforms
//! (GitHub, LinkedIn, etc.) and export verifiable credentials for use elsewhere.

use chrono::{DateTime, Utc};
use secp256k1::ecdsa::Signature;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;

/// Signature scheme used for credential proofs.
///
/// The proof is an ECDSA signature over the SHA-256 digest of the credential's
/// canonical byte serialization (see [`canonical_credential_bytes`]) on the
/// secp256k1 curve, following the W3C `EcdsaSecp256k1Signature2019` suite
/// conventions.
const PROOF_TYPE_SECP256K1: &str = "EcdsaSecp256k1Signature2019";

/// External reputation source
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExternalReputation {
    /// Unique identifier for this record.
    pub id: Uuid,
    /// Local user this external reputation is linked to.
    pub user_id: Uuid,
    /// String identifier for the source platform.
    pub platform: String,
    /// User's ID on the source platform.
    pub platform_user_id: String,
    /// Raw reputation data from the platform.
    pub reputation_data: serde_json::Value,
    /// Score imported from the platform (mapped to 0–1000).
    pub imported_score: i32,
    /// Whether this record has been independently verified.
    pub verified: bool,
    /// Timestamp when verification was completed.
    pub verified_at: Option<DateTime<Utc>>,
    /// Timestamp when this record was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when this record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Platform type for reputation import
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// GitHub developer platform.
    GitHub,
    /// LinkedIn professional network.
    LinkedIn,
    /// Stack Overflow Q&A community.
    StackOverflow,
    /// Twitter/X social platform.
    Twitter,
    /// Discord chat and community platform.
    Discord,
    /// Any other platform identified by name.
    Custom(String),
}

/// GitHub reputation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReputation {
    /// GitHub username.
    pub username: String,
    /// Number of followers on GitHub.
    pub followers: i32,
    /// Number of public repositories.
    pub public_repos: i32,
    /// Total stars received across all repositories.
    pub total_stars: i32,
    /// Total forks of the user's repositories.
    pub total_forks: i32,
    /// Number of contributions made in the last calendar year.
    pub contributions_last_year: i32,
    /// Age of the account in days.
    pub account_age_days: i32,
    /// Most-used programming languages.
    pub top_languages: Vec<String>,
}

/// LinkedIn reputation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedInReputation {
    /// LinkedIn profile identifier.
    pub profile_id: String,
    /// Number of LinkedIn connections.
    pub connections: i32,
    /// Total skill endorsements received.
    pub endorsements: i32,
    /// Number of written recommendations received.
    pub recommendations: i32,
    /// Years of professional experience listed.
    pub years_experience: i32,
    /// Skills listed on the profile.
    pub skills: Vec<String>,
}

/// Stack Overflow reputation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackOverflowReputation {
    /// Stack Overflow numeric user ID.
    pub user_id: i32,
    /// Stack Overflow reputation score.
    pub reputation: i32,
    /// Number of gold badges earned.
    pub gold_badges: i32,
    /// Number of silver badges earned.
    pub silver_badges: i32,
    /// Number of bronze badges earned.
    pub bronze_badges: i32,
    /// Total number of answers posted.
    pub answers: i32,
    /// Total number of questions posted.
    pub questions: i32,
    /// Tags the user has the most activity in.
    pub top_tags: Vec<String>,
}

/// Verifiable credential for exporting reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    /// JSON-LD context URIs.
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    /// Credential type array (e.g. `["VerifiableCredential", "ReputationCredential"]`).
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    /// Unique URI identifying this credential.
    pub id: String,
    /// DID or URI of the issuing authority.
    pub issuer: String,
    /// Timestamp when the credential was issued.
    pub issuance_date: DateTime<Utc>,
    /// Optional timestamp after which the credential is no longer valid.
    pub expiration_date: Option<DateTime<Utc>>,
    /// Claims about the credential subject.
    pub credential_subject: CredentialSubject,
    /// Cryptographic proof of authenticity.
    pub proof: CredentialProof,
}

/// Subject of the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSubject {
    /// DID or identifier of the subject.
    pub id: String,
    /// Reputation score at time of credential issuance.
    pub reputation_score: i32,
    /// Reputation tier string at time of credential issuance.
    pub reputation_tier: String,
    /// Platform identifier where the score originated.
    pub platform: String,
    /// Whether the score has been independently verified.
    pub verified: bool,
    /// Date the credential reflects the state of.
    pub as_of_date: DateTime<Utc>,
}

/// Cryptographic proof for the credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialProof {
    /// Proof type (e.g. `"EcdsaSecp256k1Signature2019"`).
    #[serde(rename = "type")]
    pub proof_type: String,
    /// Timestamp when the proof was created.
    pub created: DateTime<Utc>,
    /// Identifier of the key used to create the proof.
    ///
    /// For [`PROOF_TYPE_SECP256K1`] proofs this is a self-describing
    /// `did:key:secp256k1:<hex-compressed-public-key>#key-1` URI, so verifiers
    /// can recover the signer's public key without an external key registry.
    pub verification_method: String,
    /// Encoded proof value.
    ///
    /// For [`PROOF_TYPE_SECP256K1`] proofs this is the lowercase-hex DER encoding
    /// of the ECDSA signature over the credential's canonical digest.
    pub proof_value: String,
}

/// Request to import reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReputationRequest {
    /// Local user ID to associate with the imported reputation.
    pub user_id: Uuid,
    /// Source platform to import from.
    pub platform: Platform,
    /// User identifier on the source platform.
    pub platform_user_id: String,
    /// OAuth access token for accessing the platform's API, if required.
    pub access_token: Option<String>,
    /// Real, caller-supplied reputation data for the platform.
    ///
    /// This crate ships without an HTTP client, so it never fetches reputation
    /// from external services on its own. The caller is responsible for
    /// obtaining the data (e.g. via a platform API in an upstream service) and
    /// injecting it here. When this is `None`,
    /// [`PortabilityService::import_reputation`] returns
    /// [`ReputationError::ExternalDataUnavailable`] instead of inventing data.
    #[serde(default)]
    pub reputation_data: Option<serde_json::Value>,
}

/// Request to export reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportReputationRequest {
    /// User whose reputation should be exported.
    pub user_id: Uuid,
    /// Whether to include a cryptographic proof in the export.
    pub include_proof: bool,
    /// Optional credential expiration in days from now.
    pub expiration_days: Option<i32>,
}

/// Import result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// ID assigned to the newly created external reputation record.
    pub external_reputation_id: Uuid,
    /// Score value imported from the external platform.
    pub imported_score: i32,
    /// Whether manual verification is required before the score is applied.
    pub verification_required: bool,
    /// URL to visit for manual verification, if required.
    pub verification_url: Option<String>,
}

/// Service for reputation portability
pub struct PortabilityService {
    pool: PgPool,
}

impl PortabilityService {
    /// Create a new [`PortabilityService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Import reputation from an external platform.
    ///
    /// Reputation data must be supplied explicitly via
    /// [`ImportReputationRequest::reputation_data`]; this crate has no HTTP
    /// client and never fetches or fabricates data on its own. When no data is
    /// provided the call fails with [`ReputationError::ExternalDataUnavailable`].
    pub async fn import_reputation(
        &self,
        request: ImportReputationRequest,
    ) -> Result<ImportResult, ReputationError> {
        let platform_str = request.platform.to_string();

        // Use caller-supplied real reputation data. This crate has no HTTP
        // client, so it never invents data: when none is provided, surface an
        // honest typed error describing how to obtain it.
        let reputation_data = match request.reputation_data {
            Some(data) => data,
            None => {
                self.fetch_platform_data(
                    &request.platform,
                    &request.platform_user_id,
                    request.access_token,
                )
                .await?
            }
        };

        // Calculate imported score based on platform data
        let imported_score = self.calculate_imported_score(&request.platform, &reputation_data);

        let id = Uuid::new_v4();

        let external_rep = sqlx::query_as::<_, ExternalReputation>(
            r#"
            INSERT INTO external_reputations (
                id, user_id, platform, platform_user_id, reputation_data,
                imported_score, verified, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, false, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(request.user_id)
        .bind(&platform_str)
        .bind(&request.platform_user_id)
        .bind(&reputation_data)
        .bind(imported_score)
        .fetch_one(&self.pool)
        .await?;

        // Generate verification URL
        let verification_url =
            format!("https://api.kaccy.io/reputation/verify/{}", external_rep.id);

        Ok(ImportResult {
            external_reputation_id: external_rep.id,
            imported_score,
            verification_required: true,
            verification_url: Some(verification_url),
        })
    }

    /// Verify an imported reputation
    pub async fn verify_import(&self, external_rep_id: Uuid) -> Result<(), ReputationError> {
        sqlx::query(
            r#"
            UPDATE external_reputations
            SET verified = true, verified_at = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(external_rep_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Export reputation as a verifiable credential.
    ///
    /// When `request.include_proof` is set, the returned credential carries a
    /// real ECDSA (secp256k1) signature over the SHA-256 digest of its canonical
    /// byte serialization, produced with the caller-supplied `signing_key`. The
    /// resulting proof can be checked with [`PortabilityService::verify_proof`]
    /// or the free function [`verify_credential`].
    ///
    /// The signing secret key is never generated or stored by this service; it
    /// is supplied by the caller so that key custody stays with the issuer.
    pub async fn export_reputation(
        &self,
        request: ExportReputationRequest,
        signing_key: &SecretKey,
    ) -> Result<VerifiableCredential, ReputationError> {
        // Get user's current reputation
        let score = sqlx::query_as::<_, (i32, String)>(
            r#"
            SELECT total_score,
                   CASE
                       WHEN total_score >= 800 THEN 'Platinum'
                       WHEN total_score >= 600 THEN 'Gold'
                       WHEN total_score >= 400 THEN 'Silver'
                       WHEN total_score >= 200 THEN 'Bronze'
                       ELSE 'Restricted'
                   END as tier
            FROM reputation_scores
            WHERE user_id = $1
            "#,
        )
        .bind(request.user_id)
        .fetch_one(&self.pool)
        .await?;

        let (total_score, tier) = score;

        let credential_id = Uuid::new_v4();
        let now = Utc::now();
        let expiration = request
            .expiration_days
            .map(|days| now + chrono::Duration::days(days as i64));

        // Build the credential body first with an unsigned (empty) proof; the
        // real signature is computed over the canonical serialization of
        // everything except the proof, then written back in.
        let mut credential = VerifiableCredential {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://kaccy.io/credentials/v1".to_string(),
            ],
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "ReputationCredential".to_string(),
            ],
            id: format!("https://kaccy.io/credentials/{}", credential_id),
            issuer: "did:kaccy:issuer".to_string(),
            issuance_date: now,
            expiration_date: expiration,
            credential_subject: CredentialSubject {
                id: format!("did:kaccy:user:{}", request.user_id),
                reputation_score: total_score,
                reputation_tier: tier,
                platform: "Kaccy Protocol".to_string(),
                verified: true,
                as_of_date: now,
            },
            proof: CredentialProof {
                proof_type: PROOF_TYPE_SECP256K1.to_string(),
                created: now,
                verification_method: String::new(),
                proof_value: String::new(),
            },
        };

        if request.include_proof {
            credential.proof = Self::generate_proof(&credential, signing_key, now)?;
        }

        // Store export record
        self.store_export_record(request.user_id, &credential)
            .await?;

        Ok(credential)
    }

    /// Get all imported reputations for a user
    pub async fn get_imports(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ExternalReputation>, ReputationError> {
        let imports = sqlx::query_as::<_, ExternalReputation>(
            "SELECT * FROM external_reputations WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(imports)
    }

    /// Get verified imports only
    pub async fn get_verified_imports(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ExternalReputation>, ReputationError> {
        let imports = sqlx::query_as::<_, ExternalReputation>(
            "SELECT * FROM external_reputations WHERE user_id = $1 AND verified = true ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(imports)
    }

    /// Calculate total imported score for a user
    pub async fn calculate_total_imported_score(
        &self,
        user_id: Uuid,
    ) -> Result<i32, ReputationError> {
        let total = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT SUM(imported_score) FROM external_reputations WHERE user_id = $1 AND verified = true",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(total.unwrap_or(0) as i32)
    }

    // Helper methods

    /// Fallback used by [`Self::import_reputation`] when the caller did not
    /// supply real reputation data.
    ///
    /// This crate intentionally has no HTTP client, so it cannot — and must not
    /// — fetch reputation from external platforms on its own. Rather than
    /// fabricate plausible-looking data, this always returns a typed
    /// [`ReputationError::ExternalDataUnavailable`] error so that callers are
    /// forced to inject genuine data via
    /// [`ImportReputationRequest::reputation_data`].
    async fn fetch_platform_data(
        &self,
        platform: &Platform,
        _platform_user_id: &str,
        _access_token: Option<String>,
    ) -> Result<serde_json::Value, ReputationError> {
        Err(ReputationError::ExternalDataUnavailable {
            platform: platform.to_string(),
            reason: "no real data source is configured; this crate has no HTTP \
                     client, so reputation data must be supplied via \
                     `ImportReputationRequest::reputation_data`"
                .to_string(),
        })
    }

    fn calculate_imported_score(&self, platform: &Platform, data: &serde_json::Value) -> i32 {
        match platform {
            Platform::GitHub => {
                if let Ok(github) = serde_json::from_value::<GitHubReputation>(data.clone()) {
                    // Simple scoring formula
                    let stars_score = (github.total_stars / 10).min(100);
                    let repos_score = (github.public_repos * 2).min(50);
                    let contrib_score = (github.contributions_last_year / 10).min(100);
                    let follower_score = (github.followers / 2).min(50);
                    stars_score + repos_score + contrib_score + follower_score
                } else {
                    0
                }
            }
            Platform::LinkedIn => {
                if let Ok(linkedin) = serde_json::from_value::<LinkedInReputation>(data.clone()) {
                    let connection_score = (linkedin.connections / 10).min(50);
                    let endorsement_score = (linkedin.endorsements * 2).min(100);
                    let rec_score = (linkedin.recommendations * 5).min(50);
                    let exp_score = (linkedin.years_experience * 10).min(100);
                    connection_score + endorsement_score + rec_score + exp_score
                } else {
                    0
                }
            }
            Platform::StackOverflow => {
                if let Ok(so) = serde_json::from_value::<StackOverflowReputation>(data.clone()) {
                    (so.reputation / 10).min(300)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Produce a real cryptographic proof for `credential`.
    ///
    /// The proof is an ECDSA signature on the secp256k1 curve over the SHA-256
    /// digest of the credential's canonical byte serialization (see
    /// [`canonical_credential_bytes`]). The canonical form deliberately excludes
    /// the proof itself, so the value being signed is fully determined by the
    /// credential's claims. The signer's compressed public key is recorded in
    /// [`CredentialProof::verification_method`] and the DER-encoded signature is
    /// recorded as a hex string in [`CredentialProof::proof_value`], which makes
    /// the proof independently verifiable via [`verify_credential`].
    fn generate_proof(
        credential: &VerifiableCredential,
        signing_key: &SecretKey,
        created: DateTime<Utc>,
    ) -> Result<CredentialProof, ReputationError> {
        let secp = Secp256k1::new();
        let public_key = signing_key.public_key(&secp);

        let digest = credential_digest(credential)?;
        let message = Message::from_digest(digest);
        let signature = secp.sign_ecdsa(message, signing_key);

        Ok(CredentialProof {
            proof_type: PROOF_TYPE_SECP256K1.to_string(),
            created,
            // `did:key`-style verification method carrying the compressed
            // secp256k1 public key in hex so the proof is self-describing.
            verification_method: format!("did:key:secp256k1:{public_key}#key-1"),
            proof_value: signature.to_string(),
        })
    }

    /// Verify the cryptographic proof attached to `credential`.
    ///
    /// Returns `Ok(true)` when the signature in the proof was produced over the
    /// credential's canonical serialization by the key advertised in the proof's
    /// verification method, and `Ok(false)` when the signature does not match
    /// (e.g. because any credential field was tampered with). A malformed proof
    /// (unparseable key or signature, or an unsupported proof type) yields a
    /// typed [`ReputationError::Cryptographic`] error rather than a silent
    /// `false`.
    pub fn verify_proof(credential: &VerifiableCredential) -> Result<bool, ReputationError> {
        verify_credential(credential)
    }

    async fn store_export_record(
        &self,
        user_id: Uuid,
        credential: &VerifiableCredential,
    ) -> Result<(), ReputationError> {
        let credential_json = serde_json::to_value(credential).map_err(|e| {
            ReputationError::Validation(format!("Failed to serialize credential: {e}"))
        })?;

        sqlx::query(
            r#"
            INSERT INTO reputation_exports (
                id, user_id, credential_data, exported_at
            ) VALUES ($1, $2, $3, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(credential_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Build the canonical byte serialization of a credential for signing/verifying.
///
/// The canonical form is the JSON serialization of the credential with its
/// `proof` member removed. Object keys are emitted in sorted order because this
/// crate's `serde_json` is built without the `preserve_order` feature, so its
/// object map is a `BTreeMap`; this makes the encoding deterministic and
/// independent of struct field declaration order. Two credentials with the same
/// claims therefore always produce identical canonical bytes, while changing any
/// claim changes the bytes (and hence the digest and signature).
pub fn canonical_credential_bytes(
    credential: &VerifiableCredential,
) -> Result<Vec<u8>, ReputationError> {
    let mut value = serde_json::to_value(credential).map_err(|e| {
        ReputationError::Cryptographic(format!("failed to serialize credential: {e}"))
    })?;

    if let Some(object) = value.as_object_mut() {
        object.remove("proof");
    }

    serde_json::to_vec(&value).map_err(|e| {
        ReputationError::Cryptographic(format!("failed to encode canonical credential: {e}"))
    })
}

/// Compute the SHA-256 digest of a credential's canonical serialization.
fn credential_digest(credential: &VerifiableCredential) -> Result<[u8; 32], ReputationError> {
    let bytes = canonical_credential_bytes(credential)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher.finalize().into())
}

/// Extract the compressed secp256k1 public key from a proof's verification
/// method of the form `did:key:secp256k1:<hex>#key-1`.
fn extract_public_key(verification_method: &str) -> Result<PublicKey, ReputationError> {
    let after_prefix = verification_method
        .strip_prefix("did:key:secp256k1:")
        .ok_or_else(|| {
            ReputationError::Cryptographic(format!(
                "unsupported verification method: {verification_method}"
            ))
        })?;

    let hex_key = after_prefix
        .split('#')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ReputationError::Cryptographic(
                "verification method is missing a public key".to_string(),
            )
        })?;

    hex_key.parse::<PublicKey>().map_err(|e| {
        ReputationError::Cryptographic(format!("invalid public key in verification method: {e}"))
    })
}

/// Verify the cryptographic proof attached to a credential.
///
/// Returns `Ok(true)` when the proof's signature validates against the
/// credential's canonical digest using the public key advertised in the proof's
/// verification method, and `Ok(false)` when verification fails because the
/// signature does not correspond to the (possibly tampered) credential content.
/// Structural problems with the proof — an unsupported proof type, a missing or
/// unparseable key, or a malformed signature — return a typed
/// [`ReputationError::Cryptographic`] error so that an unsigned or corrupt
/// credential is never mistaken for a validly-signed one.
pub fn verify_credential(credential: &VerifiableCredential) -> Result<bool, ReputationError> {
    if credential.proof.proof_type != PROOF_TYPE_SECP256K1 {
        return Err(ReputationError::Cryptographic(format!(
            "unsupported proof type: {}",
            credential.proof.proof_type
        )));
    }

    if credential.proof.proof_value.is_empty() {
        return Err(ReputationError::Cryptographic(
            "credential has no proof value (it is unsigned)".to_string(),
        ));
    }

    let public_key = extract_public_key(&credential.proof.verification_method)?;

    let signature = credential
        .proof
        .proof_value
        .parse::<Signature>()
        .map_err(|e| ReputationError::Cryptographic(format!("invalid signature: {e}")))?;

    let digest = credential_digest(credential)?;
    let message = Message::from_digest(digest);

    let secp = Secp256k1::new();
    Ok(secp.verify_ecdsa(message, &signature, &public_key).is_ok())
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::GitHub => write!(f, "github"),
            Platform::LinkedIn => write!(f, "linkedin"),
            Platform::StackOverflow => write!(f, "stackoverflow"),
            Platform::Twitter => write!(f, "twitter"),
            Platform::Discord => write!(f, "discord"),
            Platform::Custom(name) => write!(f, "{name}"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github" => Ok(Platform::GitHub),
            "linkedin" => Ok(Platform::LinkedIn),
            "stackoverflow" | "stack_overflow" | "stack overflow" => Ok(Platform::StackOverflow),
            "twitter" => Ok(Platform::Twitter),
            "discord" => Ok(Platform::Discord),
            custom => Ok(Platform::Custom(custom.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::GitHub.to_string(), "github");
        assert_eq!(Platform::LinkedIn.to_string(), "linkedin");
        assert_eq!(Platform::StackOverflow.to_string(), "stackoverflow");
        assert_eq!(Platform::Custom("custom".to_string()).to_string(), "custom");
    }

    #[test]
    fn test_platform_from_str_valid() {
        assert_eq!("github".parse::<Platform>().unwrap(), Platform::GitHub);
        assert_eq!("linkedin".parse::<Platform>().unwrap(), Platform::LinkedIn);
        assert_eq!(
            "stackoverflow".parse::<Platform>().unwrap(),
            Platform::StackOverflow
        );
        assert_eq!("twitter".parse::<Platform>().unwrap(), Platform::Twitter);
        assert_eq!("discord".parse::<Platform>().unwrap(), Platform::Discord);
    }

    #[test]
    fn test_platform_from_str_variants() {
        // Test stack overflow variants
        assert_eq!(
            "stack_overflow".parse::<Platform>().unwrap(),
            Platform::StackOverflow
        );
        assert_eq!(
            "stack overflow".parse::<Platform>().unwrap(),
            Platform::StackOverflow
        );
    }

    #[test]
    fn test_platform_from_str_case_insensitive() {
        assert_eq!("GITHUB".parse::<Platform>().unwrap(), Platform::GitHub);
        assert_eq!("LinkedIn".parse::<Platform>().unwrap(), Platform::LinkedIn);
        assert_eq!(
            "STACKOVERFLOW".parse::<Platform>().unwrap(),
            Platform::StackOverflow
        );
    }

    #[test]
    fn test_platform_from_str_custom() {
        // Unknown platforms should become Custom
        assert_eq!(
            "myplatform".parse::<Platform>().unwrap(),
            Platform::Custom("myplatform".to_string())
        );
        assert_eq!(
            "reddit".parse::<Platform>().unwrap(),
            Platform::Custom("reddit".to_string())
        );
    }

    // --- Portability cryptography / data-injection tests ---

    /// A fixed, valid secp256k1 secret key used only as a deterministic test
    /// fixture. This is NOT a production key: real callers supply their own key
    /// to [`PortabilityService::export_reputation`].
    fn test_signing_key() -> SecretKey {
        // 32 non-zero bytes that form a valid secp256k1 scalar.
        let bytes: [u8; 32] = [
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x20,
        ];
        SecretKey::from_byte_array(bytes).expect("fixture bytes must be a valid secret key")
    }

    /// Build a sample credential whose proof is signed with `signing_key`.
    fn signed_sample_credential(signing_key: &SecretKey) -> VerifiableCredential {
        let now = Utc::now();
        let mut credential = VerifiableCredential {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://kaccy.io/credentials/v1".to_string(),
            ],
            credential_type: vec![
                "VerifiableCredential".to_string(),
                "ReputationCredential".to_string(),
            ],
            id: format!("https://kaccy.io/credentials/{}", Uuid::new_v4()),
            issuer: "did:kaccy:issuer".to_string(),
            issuance_date: now,
            expiration_date: None,
            credential_subject: CredentialSubject {
                id: format!("did:kaccy:user:{}", Uuid::new_v4()),
                reputation_score: 742,
                reputation_tier: "Gold".to_string(),
                platform: "Kaccy Protocol".to_string(),
                verified: true,
                as_of_date: now,
            },
            proof: CredentialProof {
                proof_type: PROOF_TYPE_SECP256K1.to_string(),
                created: now,
                verification_method: String::new(),
                proof_value: String::new(),
            },
        };
        credential.proof = PortabilityService::generate_proof(&credential, signing_key, now)
            .expect("signing the sample credential must succeed");
        credential
    }

    #[test]
    fn test_proof_sign_verify_round_trip() {
        let key = test_signing_key();
        let credential = signed_sample_credential(&key);

        // The proof must carry a real (non-placeholder) signature and a
        // self-describing verification method holding the signer's public key.
        assert_eq!(credential.proof.proof_type, PROOF_TYPE_SECP256K1);
        assert!(!credential.proof.proof_value.is_empty());
        assert_ne!(
            credential.proof.proof_value,
            "simulated_signature_placeholder"
        );
        assert_ne!(credential.proof.proof_value, "placeholder");
        assert!(credential
            .proof
            .verification_method
            .starts_with("did:key:secp256k1:"));

        // A freshly signed credential must verify true through both entry points.
        assert!(verify_credential(&credential).expect("verification must not error"));
        assert!(
            PortabilityService::verify_proof(&credential).expect("verification must not error")
        );

        // The recorded verification method must match the signing key's public key.
        let secp = Secp256k1::new();
        let expected_pk = key.public_key(&secp);
        assert_eq!(
            credential.proof.verification_method,
            format!("did:key:secp256k1:{expected_pk}#key-1")
        );
    }

    #[test]
    fn test_proof_tamper_detection_score() {
        let key = test_signing_key();
        let mut credential = signed_sample_credential(&key);

        // Tampering with a signed claim (the score) must make verification fail.
        credential.credential_subject.reputation_score += 1;
        assert!(
            !verify_credential(&credential).expect("verification must not error"),
            "tampered score must not verify"
        );
    }

    #[test]
    fn test_proof_tamper_detection_subject_and_tier() {
        let key = test_signing_key();

        // Tamper with the subject identifier.
        let mut credential = signed_sample_credential(&key);
        credential.credential_subject.id = "did:kaccy:user:attacker".to_string();
        assert!(
            !verify_credential(&credential).expect("verification must not error"),
            "tampered subject id must not verify"
        );

        // Tamper with the tier string.
        let mut credential = signed_sample_credential(&key);
        credential.credential_subject.reputation_tier = "Platinum".to_string();
        assert!(
            !verify_credential(&credential).expect("verification must not error"),
            "tampered tier must not verify"
        );

        // Tamper with the issuer.
        let mut credential = signed_sample_credential(&key);
        credential.issuer = "did:kaccy:rogue".to_string();
        assert!(
            !verify_credential(&credential).expect("verification must not error"),
            "tampered issuer must not verify"
        );
    }

    #[test]
    fn test_proof_wrong_key_fails() {
        let key = test_signing_key();
        let credential = signed_sample_credential(&key);

        // Re-point the verification method at a different (valid) public key.
        let other_bytes: [u8; 32] = [0x42; 32];
        let other_key =
            SecretKey::from_byte_array(other_bytes).expect("other fixture key must be valid");
        let secp = Secp256k1::new();
        let other_pk = other_key.public_key(&secp);

        let mut tampered = credential;
        tampered.proof.verification_method = format!("did:key:secp256k1:{other_pk}#key-1");

        assert!(
            !verify_credential(&tampered).expect("verification must not error"),
            "signature must not verify under a different public key"
        );
    }

    #[test]
    fn test_verify_unsigned_credential_errors() {
        let key = test_signing_key();
        let mut credential = signed_sample_credential(&key);

        // An empty proof value (an unsigned credential) is a structural error,
        // never a silent `false` that could be mistaken for "checked".
        credential.proof.proof_value = String::new();
        let err = verify_credential(&credential).expect_err("unsigned credential must error");
        assert!(matches!(err, ReputationError::Cryptographic(_)));
    }

    #[test]
    fn test_verify_malformed_signature_errors() {
        let key = test_signing_key();
        let mut credential = signed_sample_credential(&key);

        credential.proof.proof_value = "not-a-valid-hex-signature".to_string();
        let err = verify_credential(&credential).expect_err("malformed signature must error");
        assert!(matches!(err, ReputationError::Cryptographic(_)));
    }

    #[test]
    fn test_canonical_bytes_exclude_proof_and_are_deterministic() {
        let key = test_signing_key();
        let credential = signed_sample_credential(&key);

        let bytes_a =
            canonical_credential_bytes(&credential).expect("canonicalization must succeed");
        let bytes_b =
            canonical_credential_bytes(&credential).expect("canonicalization must succeed");
        assert_eq!(bytes_a, bytes_b, "canonical bytes must be deterministic");

        // The canonical form must not contain the proof at all, so re-signing or
        // changing the proof value cannot alter what is signed.
        let text = String::from_utf8(bytes_a).expect("canonical bytes are valid UTF-8 JSON");
        assert!(!text.contains("\"proof\""));
        assert!(!text.contains(&credential.proof.proof_value));
    }

    #[tokio::test]
    async fn test_fetch_platform_data_unconfigured_returns_honest_error() {
        // A lazily-connected pool never opens a socket here because
        // `fetch_platform_data` does not touch the database before returning.
        let pool = PgPool::connect_lazy("postgres://localhost/kaccy_test_unused")
            .expect("lazy pool construction must succeed");
        let service = PortabilityService::new(pool);

        let result = service
            .fetch_platform_data(&Platform::GitHub, "octocat", None)
            .await;

        match result {
            Err(ReputationError::ExternalDataUnavailable { platform, .. }) => {
                assert_eq!(platform, "github");
            }
            other => panic!("expected ExternalDataUnavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_import_score_from_explicit_real_data() {
        // `calculate_imported_score` is the scoring core used by
        // `import_reputation`; here we feed it explicitly-provided real platform
        // data and check it derives a deterministic, non-trivial score.
        // `connect_lazy` needs a Tokio context (it spawns a maintenance task)
        // but never opens a socket, and `calculate_imported_score` never touches
        // the pool, so no database is required.
        let pool = PgPool::connect_lazy("postgres://localhost/kaccy_test_unused")
            .expect("lazy pool construction must succeed");
        let service = PortabilityService::new(pool);

        let github_data = serde_json::json!({
            "username": "octocat",
            "followers": 120,
            "public_repos": 40,
            "total_stars": 800,
            "total_forks": 150,
            "contributions_last_year": 1200,
            "account_age_days": 2000,
            "top_languages": ["Rust", "Go"]
        });

        // stars: min(800/10,100)=80; repos: min(40*2,50)=50;
        // contrib: min(1200/10,100)=100; followers: min(120/2,50)=50 => 280.
        let score = service.calculate_imported_score(&Platform::GitHub, &github_data);
        assert_eq!(score, 280);

        let so_data = serde_json::json!({
            "user_id": 42,
            "reputation": 5000,
            "gold_badges": 3,
            "silver_badges": 12,
            "bronze_badges": 40,
            "answers": 210,
            "questions": 30,
            "top_tags": ["rust"]
        });
        // min(5000/10, 300) = 300.
        assert_eq!(
            service.calculate_imported_score(&Platform::StackOverflow, &so_data),
            300
        );
    }

    #[test]
    fn test_import_request_round_trips_with_injected_data() {
        // The request type must be able to carry caller-supplied real data so
        // that data is injected, not invented.
        let request = ImportReputationRequest {
            user_id: Uuid::new_v4(),
            platform: Platform::GitHub,
            platform_user_id: "octocat".to_string(),
            access_token: None,
            reputation_data: Some(serde_json::json!({ "followers": 10 })),
        };
        let encoded = serde_json::to_string(&request).expect("request must serialize");
        let decoded: ImportReputationRequest =
            serde_json::from_str(&encoded).expect("request must deserialize");
        assert_eq!(decoded.reputation_data, request.reputation_data);

        // Backward compatibility: a payload without the new field decodes with
        // `reputation_data == None` (the `#[serde(default)]`), which is exactly
        // the case that triggers the honest unavailable-data error. The
        // `Platform` enum uses snake_case, so `GitHub` is encoded as `git_hub`.
        let legacy = r#"{"user_id":"00000000-0000-0000-0000-000000000000","platform":"git_hub","platform_user_id":"octocat","access_token":null}"#;
        let decoded_legacy: ImportReputationRequest =
            serde_json::from_str(legacy).expect("legacy request must deserialize");
        assert!(decoded_legacy.reputation_data.is_none());
        assert_eq!(decoded_legacy.platform, Platform::GitHub);
    }
}
