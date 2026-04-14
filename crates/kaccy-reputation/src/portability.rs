//! Reputation Portability for importing and exporting reputation data
//!
//! This module enables users to import reputation from external platforms
//! (GitHub, LinkedIn, etc.) and export verifiable credentials for use elsewhere.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;

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
    /// Proof type (e.g. `"Ed25519Signature2020"`).
    #[serde(rename = "type")]
    pub proof_type: String,
    /// Timestamp when the proof was created.
    pub created: DateTime<Utc>,
    /// Identifier of the key used to create the proof.
    pub verification_method: String,
    /// Base64-encoded proof value.
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

    /// Import reputation from an external platform
    pub async fn import_reputation(
        &self,
        request: ImportReputationRequest,
    ) -> Result<ImportResult, ReputationError> {
        let platform_str = request.platform.to_string();

        // Fetch reputation data from platform
        let reputation_data = self
            .fetch_platform_data(
                &request.platform,
                &request.platform_user_id,
                request.access_token,
            )
            .await?;

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

    /// Export reputation as a verifiable credential
    pub async fn export_reputation(
        &self,
        request: ExportReputationRequest,
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

        let credential = VerifiableCredential {
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
            proof: if request.include_proof {
                self.generate_proof(&credential_id, &request.user_id, total_score, now)
            } else {
                CredentialProof {
                    proof_type: "Ed25519Signature2020".to_string(),
                    created: now,
                    verification_method: "https://kaccy.io/keys/1".to_string(),
                    proof_value: "placeholder".to_string(),
                }
            },
        };

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

    async fn fetch_platform_data(
        &self,
        platform: &Platform,
        _platform_user_id: &str,
        _access_token: Option<String>,
    ) -> Result<serde_json::Value, ReputationError> {
        // In a real implementation, this would call external APIs
        // For now, return placeholder data
        match platform {
            Platform::GitHub => Ok(serde_json::json!({
                "username": "example",
                "followers": 100,
                "public_repos": 50,
                "total_stars": 500,
                "total_forks": 100,
                "contributions_last_year": 1000,
                "account_age_days": 1000,
                "top_languages": ["Rust", "TypeScript"]
            })),
            Platform::LinkedIn => Ok(serde_json::json!({
                "profile_id": "example",
                "connections": 500,
                "endorsements": 50,
                "recommendations": 10,
                "years_experience": 5,
                "skills": ["Software Development", "Leadership"]
            })),
            Platform::StackOverflow => Ok(serde_json::json!({
                "user_id": 12345,
                "reputation": 5000,
                "gold_badges": 2,
                "silver_badges": 10,
                "bronze_badges": 50,
                "answers": 200,
                "questions": 50,
                "top_tags": ["rust", "typescript", "javascript"]
            })),
            _ => Ok(serde_json::json!({})),
        }
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

    fn generate_proof(
        &self,
        _credential_id: &Uuid,
        _user_id: &Uuid,
        _score: i32,
        created: DateTime<Utc>,
    ) -> CredentialProof {
        // In a real implementation, this would use cryptographic signing
        CredentialProof {
            proof_type: "Ed25519Signature2020".to_string(),
            created,
            verification_method: "https://kaccy.io/keys/1".to_string(),
            proof_value: "simulated_signature_placeholder".to_string(),
        }
    }

    async fn store_export_record(
        &self,
        user_id: Uuid,
        credential: &VerifiableCredential,
    ) -> Result<(), ReputationError> {
        let credential_json = serde_json::to_value(credential).map_err(|e| {
            ReputationError::Validation(format!("Failed to serialize credential: {}", e))
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

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::GitHub => write!(f, "github"),
            Platform::LinkedIn => write!(f, "linkedin"),
            Platform::StackOverflow => write!(f, "stackoverflow"),
            Platform::Twitter => write!(f, "twitter"),
            Platform::Discord => write!(f, "discord"),
            Platform::Custom(name) => write!(f, "{}", name),
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
}
