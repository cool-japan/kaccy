//! Soulbound Token (SBT) implementation for on-chain reputation
//!
//! This module provides functionality for minting and managing
//! non-transferable reputation tokens on-chain.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;
use crate::score::ReputationScore;
use crate::tier::ReputationTier;

/// Soulbound Token representing on-chain reputation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SoulboundToken {
    pub id: Uuid,
    pub user_id: Uuid,
    /// On-chain token ID (after minting)
    pub token_id: Option<String>,
    /// Current reputation score
    pub score: i32,
    /// Current tier
    pub tier: String,
    /// Metadata URI pointing to JSON metadata
    pub metadata_uri: String,
    /// Whether the token has been minted on-chain
    pub minted: bool,
    /// Blockchain transaction hash of minting
    pub mint_tx_hash: Option<String>,
    /// Last time the metadata was updated on-chain
    pub last_metadata_update: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metadata for the Soulbound Token (JSON format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SBTMetadata {
    pub name: String,
    pub description: String,
    pub image: String,
    pub attributes: Vec<SBTAttribute>,
    /// External URL to user's profile
    pub external_url: Option<String>,
}

/// Attribute in the SBT metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SBTAttribute {
    pub trait_type: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_type: Option<String>,
}

/// Request to mint a new SBT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintSBTRequest {
    pub user_id: Uuid,
    pub initial_score: i32,
    pub initial_tier: ReputationTier,
}

/// Request to update SBT metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSBTRequest {
    pub user_id: Uuid,
    pub new_score: i32,
    pub new_tier: ReputationTier,
}

/// Service for managing Soulbound Tokens
pub struct SBTService {
    pool: PgPool,
}

impl SBTService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if a user has an SBT
    pub async fn has_sbt(&self, user_id: Uuid) -> Result<bool, ReputationError> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM soulbound_tokens WHERE user_id = $1)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get a user's SBT
    pub async fn get_sbt(&self, user_id: Uuid) -> Result<Option<SoulboundToken>, ReputationError> {
        let sbt = sqlx::query_as::<_, SoulboundToken>(
            "SELECT * FROM soulbound_tokens WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(sbt)
    }

    /// Create a new SBT record (before minting on-chain)
    pub async fn create_sbt(
        &self,
        request: MintSBTRequest,
    ) -> Result<SoulboundToken, ReputationError> {
        let id = Uuid::new_v4();
        let tier_str = request.initial_tier.to_string();
        let metadata_uri = format!("https://api.kaccy.io/sbt/{}/metadata.json", id);

        let sbt = sqlx::query_as::<_, SoulboundToken>(
            r#"
            INSERT INTO soulbound_tokens (
                id, user_id, score, tier, metadata_uri, minted, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, false, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(request.user_id)
        .bind(request.initial_score)
        .bind(tier_str)
        .bind(metadata_uri)
        .fetch_one(&self.pool)
        .await?;

        Ok(sbt)
    }

    /// Mark an SBT as minted on-chain
    pub async fn mark_minted(
        &self,
        user_id: Uuid,
        token_id: String,
        tx_hash: String,
    ) -> Result<SoulboundToken, ReputationError> {
        let sbt = sqlx::query_as::<_, SoulboundToken>(
            r#"
            UPDATE soulbound_tokens
            SET token_id = $1, mint_tx_hash = $2, minted = true, updated_at = NOW()
            WHERE user_id = $3
            RETURNING *
            "#,
        )
        .bind(token_id)
        .bind(tx_hash)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(sbt)
    }

    /// Update SBT metadata (score and tier)
    pub async fn update_metadata(
        &self,
        request: UpdateSBTRequest,
    ) -> Result<SoulboundToken, ReputationError> {
        let tier_str = request.new_tier.to_string();

        let sbt = sqlx::query_as::<_, SoulboundToken>(
            r#"
            UPDATE soulbound_tokens
            SET score = $1, tier = $2, last_metadata_update = NOW(), updated_at = NOW()
            WHERE user_id = $3
            RETURNING *
            "#,
        )
        .bind(request.new_score)
        .bind(tier_str)
        .bind(request.user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(sbt)
    }

    /// Generate SBT metadata JSON
    pub fn generate_metadata(
        &self,
        user_id: Uuid,
        score: &ReputationScore,
        tier: &ReputationTier,
    ) -> SBTMetadata {
        let tier_name = tier.to_string();
        let attributes = vec![
            SBTAttribute {
                trait_type: "Reputation Score".to_string(),
                value: serde_json::json!(score.overall_score),
                display_type: Some("number".to_string()),
            },
            SBTAttribute {
                trait_type: "Reputation Tier".to_string(),
                value: serde_json::json!(tier_name),
                display_type: None,
            },
            SBTAttribute {
                trait_type: "Commitment Fulfillment".to_string(),
                value: serde_json::json!(score.components.commitment_fulfillment),
                display_type: Some("number".to_string()),
            },
            SBTAttribute {
                trait_type: "Community Trust".to_string(),
                value: serde_json::json!(score.components.community_trust),
                display_type: Some("number".to_string()),
            },
            SBTAttribute {
                trait_type: "Quality Rating".to_string(),
                value: serde_json::json!(score.components.quality_rating),
                display_type: Some("number".to_string()),
            },
            SBTAttribute {
                trait_type: "Longevity".to_string(),
                value: serde_json::json!(score.components.longevity),
                display_type: Some("number".to_string()),
            },
        ];

        SBTMetadata {
            name: format!("Kaccy Reputation - {} Tier", tier_name),
            description: format!(
                "Non-transferable reputation token for Kaccy Protocol. Score: {}. This token represents the on-chain reputation of user {} and cannot be transferred.",
                score.overall_score, user_id
            ),
            image: format!(
                "https://assets.kaccy.io/sbt/{}.png",
                tier_name.to_lowercase()
            ),
            attributes,
            external_url: Some(format!("https://kaccy.io/profile/{}", user_id)),
        }
    }

    /// Get all SBTs that need metadata updates (score changed significantly)
    pub async fn get_pending_updates(
        &self,
        _min_score_delta: i32,
    ) -> Result<Vec<SoulboundToken>, ReputationError> {
        // This would typically query SBTs where the on-chain score differs from the current score
        // For now, we'll return SBTs that haven't been updated in the last 24 hours
        let sbts = sqlx::query_as::<_, SoulboundToken>(
            r#"
            SELECT * FROM soulbound_tokens
            WHERE minted = true
            AND (last_metadata_update IS NULL OR last_metadata_update < NOW() - INTERVAL '24 hours')
            LIMIT 100
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sbts)
    }

    /// Check if an SBT needs metadata update
    pub async fn needs_update(
        &self,
        user_id: Uuid,
        current_score: i32,
        threshold: i32,
    ) -> Result<bool, ReputationError> {
        let sbt = self.get_sbt(user_id).await?;

        match sbt {
            Some(sbt) if sbt.minted => {
                let score_diff = (current_score - sbt.score).abs();
                Ok(score_diff >= threshold)
            }
            _ => Ok(false),
        }
    }
}

/// Trait for on-chain SBT operations (to be implemented by blockchain layer)
#[async_trait::async_trait]
pub trait OnChainSBT {
    /// Mint a new SBT on-chain
    async fn mint(
        &self,
        user_address: &str,
        metadata_uri: &str,
    ) -> Result<(String, String), ReputationError>;

    /// Update SBT metadata URI on-chain
    async fn update_metadata(
        &self,
        token_id: &str,
        new_metadata_uri: &str,
    ) -> Result<String, ReputationError>;

    /// Verify that a token is non-transferable
    async fn is_transferable(&self, token_id: &str) -> Result<bool, ReputationError>;

    /// Get the owner of an SBT
    async fn get_owner(&self, token_id: &str) -> Result<String, ReputationError>;
}
