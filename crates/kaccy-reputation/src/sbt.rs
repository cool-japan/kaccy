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
    /// Unique identifier for this record.
    pub id: Uuid,
    /// User this SBT belongs to.
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
    /// Timestamp when this record was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when this record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Metadata for the Soulbound Token (JSON format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SBTMetadata {
    /// Display name for the token.
    pub name: String,
    /// Human-readable description of what the token represents.
    pub description: String,
    /// URI of the token's image asset.
    pub image: String,
    /// Array of trait attributes for the token.
    pub attributes: Vec<SBTAttribute>,
    /// External URL to user's profile
    pub external_url: Option<String>,
}

/// Attribute in the SBT metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SBTAttribute {
    /// Name of the trait (e.g. `"Reputation Tier"`).
    pub trait_type: String,
    /// Value of the trait.
    pub value: serde_json::Value,
    /// Optional display hint for the value (e.g. `"number"`, `"date"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_type: Option<String>,
}

/// Request to mint a new SBT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintSBTRequest {
    /// User for whom the SBT should be minted.
    pub user_id: Uuid,
    /// Reputation score at the time of minting.
    pub initial_score: i32,
    /// Reputation tier at the time of minting.
    pub initial_tier: ReputationTier,
}

/// Request to update SBT metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSBTRequest {
    /// User whose SBT should be updated.
    pub user_id: Uuid,
    /// Updated reputation score.
    pub new_score: i32,
    /// Updated reputation tier.
    pub new_tier: ReputationTier,
}

/// Generate SBT metadata JSON from score and tier (pure function, no DB access)
pub(crate) fn build_sbt_metadata(
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

/// Service for managing Soulbound Tokens
pub struct SBTService {
    pool: PgPool,
}

impl SBTService {
    /// Create a new [`SBTService`] backed by the given database pool.
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
        build_sbt_metadata(user_id, score, tier)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::ScoreComponents;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_score(overall: rust_decimal::Decimal, tier: ReputationTier) -> ReputationScore {
        ReputationScore {
            user_id: Uuid::nil(),
            overall_score: overall,
            tier,
            components: ScoreComponents {
                commitment_fulfillment: dec!(80),
                response_time: dec!(70),
                quality_rating: dec!(75),
                community_trust: dec!(60),
                longevity: dec!(50),
            },
        }
    }

    #[test]
    fn test_sbt_metadata_name_format() {
        let user_id = Uuid::nil();
        let score = make_score(dec!(650), ReputationTier::Gold);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Gold);
        assert_eq!(meta.name, "Kaccy Reputation - Gold Tier");
    }

    #[test]
    fn test_sbt_metadata_image_url_per_tier() {
        let user_id = Uuid::nil();
        for (tier, expected) in [
            (
                ReputationTier::Unverified,
                "https://assets.kaccy.io/sbt/unverified.png",
            ),
            (
                ReputationTier::Bronze,
                "https://assets.kaccy.io/sbt/bronze.png",
            ),
            (
                ReputationTier::Silver,
                "https://assets.kaccy.io/sbt/silver.png",
            ),
            (ReputationTier::Gold, "https://assets.kaccy.io/sbt/gold.png"),
            (
                ReputationTier::Platinum,
                "https://assets.kaccy.io/sbt/platinum.png",
            ),
            (
                ReputationTier::Diamond,
                "https://assets.kaccy.io/sbt/diamond.png",
            ),
        ] {
            let score = make_score(tier.min_score(), tier);
            let meta = build_sbt_metadata(user_id, &score, &tier);
            assert_eq!(meta.image, expected, "image URL mismatch for {tier:?}");
        }
    }

    #[test]
    fn test_sbt_metadata_attribute_count() {
        let user_id = Uuid::nil();
        let score = make_score(dec!(500), ReputationTier::Silver);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Silver);
        assert_eq!(meta.attributes.len(), 6, "expected 6 attributes");
    }

    #[test]
    fn test_sbt_metadata_attribute_trait_types() {
        let user_id = Uuid::nil();
        let score = make_score(dec!(500), ReputationTier::Silver);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Silver);
        let trait_types: Vec<&str> = meta
            .attributes
            .iter()
            .map(|a| a.trait_type.as_str())
            .collect();
        assert!(trait_types.contains(&"Reputation Score"));
        assert!(trait_types.contains(&"Reputation Tier"));
        assert!(trait_types.contains(&"Commitment Fulfillment"));
        assert!(trait_types.contains(&"Community Trust"));
        assert!(trait_types.contains(&"Quality Rating"));
        assert!(trait_types.contains(&"Longevity"));
    }

    #[test]
    fn test_sbt_metadata_external_url_format() {
        let user_id = Uuid::nil();
        let score = make_score(dec!(500), ReputationTier::Silver);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Silver);
        let expected_url = format!("https://kaccy.io/profile/{}", user_id);
        assert_eq!(meta.external_url, Some(expected_url));
    }

    #[test]
    fn test_sbt_metadata_description_contains_score() {
        let user_id = Uuid::nil();
        let score = make_score(dec!(750), ReputationTier::Gold);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Gold);
        assert!(
            meta.description.contains("750"),
            "description should contain score"
        );
        assert!(
            meta.description.contains(&user_id.to_string()),
            "description should contain user id"
        );
    }

    #[test]
    fn test_sbt_metadata_serde_roundtrip() {
        let user_id = Uuid::new_v4();
        let score = make_score(dec!(900), ReputationTier::Diamond);
        let meta = build_sbt_metadata(user_id, &score, &ReputationTier::Diamond);
        let json = serde_json::to_string(&meta).expect("serialize SBTMetadata");
        let restored: SBTMetadata = serde_json::from_str(&json).expect("deserialize SBTMetadata");
        assert_eq!(restored.name, meta.name);
        assert_eq!(restored.image, meta.image);
        assert_eq!(restored.external_url, meta.external_url);
        assert_eq!(restored.attributes.len(), meta.attributes.len());
    }

    #[test]
    fn test_mint_sbt_request_serde_roundtrip() {
        let req = MintSBTRequest {
            user_id: Uuid::new_v4(),
            initial_score: 500,
            initial_tier: ReputationTier::Silver,
        };
        let json = serde_json::to_string(&req).expect("serialize MintSBTRequest");
        let restored: MintSBTRequest =
            serde_json::from_str(&json).expect("deserialize MintSBTRequest");
        assert_eq!(restored.user_id, req.user_id);
        assert_eq!(restored.initial_score, req.initial_score);
        assert_eq!(restored.initial_tier, req.initial_tier);
    }

    #[test]
    fn test_sbt_needs_update_score_delta() {
        // Test the pure score-diff logic used by needs_update
        let current_score: i32 = 750;
        let sbt_score: i32 = 700;
        let threshold: i32 = 50;
        let diff = (current_score - sbt_score).abs();
        assert!(
            diff >= threshold,
            "diff {diff} should trigger update at threshold {threshold}"
        );

        let small_diff = (705 - 700_i32).abs();
        assert!(
            small_diff < threshold,
            "small diff {small_diff} should not trigger update"
        );
    }

    #[test]
    fn test_sbt_attribute_skip_none_display_type() {
        let attr_with_display = SBTAttribute {
            trait_type: "Test".to_string(),
            value: serde_json::json!(42),
            display_type: Some("number".to_string()),
        };
        let attr_without_display = SBTAttribute {
            trait_type: "Test2".to_string(),
            value: serde_json::json!("value"),
            display_type: None,
        };
        let json_with =
            serde_json::to_string(&attr_with_display).expect("serialize with display_type");
        let json_without =
            serde_json::to_string(&attr_without_display).expect("serialize without display_type");
        assert!(
            json_with.contains("display_type"),
            "should contain display_type when Some"
        );
        assert!(
            !json_without.contains("display_type"),
            "should skip display_type when None"
        );
    }

    #[test]
    fn test_mint_sbt_request_fields() {
        let user_id = Uuid::new_v4();
        let req = MintSBTRequest {
            user_id,
            initial_score: 450,
            initial_tier: ReputationTier::Silver,
        };
        assert_eq!(req.user_id, user_id);
        assert_eq!(req.initial_score, 450);
        assert_eq!(req.initial_tier, ReputationTier::Silver);
    }
}
