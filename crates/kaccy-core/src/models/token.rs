//! Token (FOT - Future Output Token) model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// FOT (Future Output Token) — core token model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Token {
    /// Unique identifier of the token
    pub token_id: Uuid,
    /// User ID of the token issuer
    pub issuer_user_id: Uuid,
    /// Ticker symbol (e.g. `$MYTOKEN`)
    pub symbol: String,
    /// Human-readable token name
    pub name: String,
    /// Optional description of the token and its purpose
    pub description: Option<String>,
    /// Maximum token supply
    pub total_supply: Decimal,
    /// Tokens currently in circulation
    pub circulating_supply: Decimal,
    /// Initial price per token in BTC
    pub initial_price_btc: Decimal,
    /// BTC price increment per unit sold on the bonding curve
    pub price_increment_btc: Decimal,
    /// Timestamp when the token was created
    pub created_at: DateTime<Utc>,
    /// Current lifecycle status of the token
    pub status: TokenStatus,
}

/// Lifecycle status of a token
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum TokenStatus {
    /// Token is active and trading is permitted
    #[default]
    Active,
    /// Token is temporarily paused
    Paused,
    /// Token has been permanently closed
    Closed,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token({}, ${}, {})",
            self.token_id, self.symbol, self.name
        )
    }
}

impl fmt::Display for TokenStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenStatus::Active => write!(f, "active"),
            TokenStatus::Paused => write!(f, "paused"),
            TokenStatus::Closed => write!(f, "closed"),
        }
    }
}

/// Request payload for creating a new token
#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    /// Ticker symbol starting with `$`
    pub symbol: String,
    /// Human-readable token name
    pub name: String,
    /// Optional token description
    pub description: Option<String>,
    /// Total supply to mint
    pub total_supply: Decimal,
    /// Initial price per token in BTC
    pub initial_price_btc: Decimal,
    /// Bonding curve price increment per token sold
    pub price_increment_btc: Decimal,
}

impl CreateTokenRequest {
    /// Validate the token creation request
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Symbol validation
        if self.symbol.is_empty() {
            return Err(ValidationError("Symbol is required".to_string()));
        }
        if self.symbol.len() > 20 {
            return Err(ValidationError(
                "Symbol must be at most 20 characters".to_string(),
            ));
        }
        if !self.symbol.starts_with('$') {
            return Err(ValidationError("Symbol must start with '$'".to_string()));
        }
        let symbol_without_prefix = &self.symbol[1..];
        if !symbol_without_prefix
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(ValidationError(
                "Symbol must contain only uppercase letters and digits after '$'".to_string(),
            ));
        }

        // Name validation
        if self.name.is_empty() {
            return Err(ValidationError("Name is required".to_string()));
        }
        if self.name.len() > 100 {
            return Err(ValidationError(
                "Name must be at most 100 characters".to_string(),
            ));
        }

        // Description validation
        if let Some(ref desc) = self.description {
            if desc.len() > 2000 {
                return Err(ValidationError(
                    "Description must be at most 2000 characters".to_string(),
                ));
            }
        }

        // Supply validation
        if self.total_supply <= dec!(0) {
            return Err(ValidationError("Total supply must be positive".to_string()));
        }
        if self.total_supply > dec!(1_000_000) {
            return Err(ValidationError(
                "Total supply must be at most 1,000,000".to_string(),
            ));
        }

        // Price validation
        if self.initial_price_btc <= dec!(0) {
            return Err(ValidationError(
                "Initial price must be positive".to_string(),
            ));
        }
        if self.initial_price_btc < dec!(0.00001) {
            return Err(ValidationError(
                "Initial price must be at least 0.00001 BTC".to_string(),
            ));
        }
        if self.price_increment_btc < dec!(0) {
            return Err(ValidationError(
                "Price increment cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

/// Token data enriched with issuer profile information
#[derive(Debug, Serialize)]
pub struct TokenWithIssuer {
    /// Core token data
    #[serde(flatten)]
    pub token: Token,
    /// Username of the token issuer
    pub issuer_username: String,
    /// Optional display name of the issuer
    pub issuer_display_name: Option<String>,
    /// Current reputation score of the issuer
    pub issuer_reputation_score: Decimal,
    /// Current bonding curve price
    pub current_price: Decimal,
}

/// Sort options for token list
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenSortBy {
    /// Sort by creation timestamp (default)
    #[default]
    CreatedAt,
    /// Sort alphabetically by symbol
    Symbol,
    /// Sort alphabetically by name
    Name,
    /// Sort by total supply
    TotalSupply,
    /// Sort by circulating supply
    CirculatingSupply,
    /// Sort by initial price
    InitialPrice,
}

/// Sort direction
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Ascending order
    Asc,
    /// Descending order (default)
    #[default]
    Desc,
}

/// Query parameters for listing tokens
#[derive(Debug, Deserialize)]
pub struct TokenListQuery {
    /// Page number (1-based)
    pub page: Option<u32>,
    /// Results per page
    pub limit: Option<u32>,
    /// Filter by token status
    pub status: Option<TokenStatus>,
    /// Text search against symbol or name
    pub search: Option<String>,
    /// Minimum initial price in BTC
    pub min_price: Option<Decimal>,
    /// Maximum initial price in BTC
    pub max_price: Option<Decimal>,
    /// Sort field
    pub sort_by: Option<TokenSortBy>,
    /// Sort direction
    pub order: Option<SortOrder>,
}

/// Extended metadata for tokens including access benefits and social links
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenMetadata {
    /// Token this metadata record belongs to.
    pub token_id: Uuid,
    /// Cover image URL for token profile
    pub cover_image_url: Option<String>,
    /// External website URL
    pub website_url: Option<String>,
    /// Twitter/X handle
    pub twitter_handle: Option<String>,
    /// Discord server invite link
    pub discord_url: Option<String>,
    /// GitHub repository URL
    pub github_url: Option<String>,
    /// YouTube channel URL
    pub youtube_url: Option<String>,
    /// Telegram group/channel URL
    pub telegram_url: Option<String>,
    /// Tags for discovery (comma-separated or JSON array)
    pub tags: Option<String>,
    /// FAQ or additional documentation (markdown)
    pub faq_content: Option<String>,
    /// Terms of service for token holders
    pub terms_of_service: Option<String>,
    /// Whether token has active utility definitions
    pub has_utilities: bool,
    /// Total number of token holder benefits
    pub utility_count: i32,
    /// Timestamp when the metadata record was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent metadata update.
    pub updated_at: DateTime<Utc>,
}

impl fmt::Display for TokenMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenMetadata(token_id={})", self.token_id)
    }
}

impl TokenMetadata {
    /// Create empty metadata for a token
    pub fn empty(token_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            token_id,
            cover_image_url: None,
            website_url: None,
            twitter_handle: None,
            discord_url: None,
            github_url: None,
            youtube_url: None,
            telegram_url: None,
            tags: None,
            faq_content: None,
            terms_of_service: None,
            has_utilities: false,
            utility_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Validate URLs in metadata
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref url) = self.website_url {
            if !url.starts_with("https://") {
                return Err(ValidationError("Website URL must use HTTPS".to_string()));
            }
        }
        if let Some(ref url) = self.discord_url {
            if !url.starts_with("https://discord.") && !url.starts_with("https://discord.gg/") {
                return Err(ValidationError(
                    "Discord URL must be a valid Discord link".to_string(),
                ));
            }
        }
        if let Some(ref url) = self.github_url {
            if !url.starts_with("https://github.com/") {
                return Err(ValidationError(
                    "GitHub URL must be a valid GitHub link".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Get tags as a vector
    pub fn tags_list(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Request payload for updating token metadata fields
#[derive(Debug, Deserialize)]
pub struct UpdateTokenMetadataRequest {
    /// New cover image URL
    pub cover_image_url: Option<String>,
    /// New website URL
    pub website_url: Option<String>,
    /// New Twitter/X handle
    pub twitter_handle: Option<String>,
    /// New Discord invite URL
    pub discord_url: Option<String>,
    /// New GitHub repository URL
    pub github_url: Option<String>,
    /// New YouTube channel URL
    pub youtube_url: Option<String>,
    /// New Telegram group URL
    pub telegram_url: Option<String>,
    /// Comma-separated tags for discovery
    pub tags: Option<String>,
    /// FAQ or documentation content (markdown)
    pub faq_content: Option<String>,
    /// Terms of service for token holders
    pub terms_of_service: Option<String>,
}

/// Type of utility/benefit for token holders
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum UtilityType {
    /// Access to exclusive content
    ContentAccess,
    /// Direct messaging with issuer
    DirectMessage,
    /// 1-on-1 calls/meetings
    OneOnOneCall,
    /// Group calls/events
    GroupCall,
    /// Discord role/access
    DiscordAccess,
    /// Early access to products/content
    EarlyAccess,
    /// Code review or feedback
    CodeReview,
    /// Mentorship sessions
    Mentorship,
    /// Custom/other utility
    #[default]
    Custom,
}

impl fmt::Display for UtilityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UtilityType::ContentAccess => write!(f, "Content Access"),
            UtilityType::DirectMessage => write!(f, "Direct Message"),
            UtilityType::OneOnOneCall => write!(f, "1:1 Call"),
            UtilityType::GroupCall => write!(f, "Group Call"),
            UtilityType::DiscordAccess => write!(f, "Discord Access"),
            UtilityType::EarlyAccess => write!(f, "Early Access"),
            UtilityType::CodeReview => write!(f, "Code Review"),
            UtilityType::Mentorship => write!(f, "Mentorship"),
            UtilityType::Custom => write!(f, "Custom"),
        }
    }
}

/// Definition of a utility/benefit for token holders
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UtilityDefinition {
    /// Unique identifier of this utility definition.
    pub utility_id: Uuid,
    /// Token this utility belongs to.
    pub token_id: Uuid,
    /// Type of utility
    pub utility_type: UtilityType,
    /// Human-readable title
    pub title: String,
    /// Detailed description of the benefit
    pub description: Option<String>,
    /// Minimum tokens required to access this utility
    pub min_tokens_required: Decimal,
    /// Maximum number of redemptions per holder (None = unlimited)
    pub max_redemptions_per_holder: Option<i32>,
    /// Maximum total redemptions (None = unlimited)
    pub max_total_redemptions: Option<i32>,
    /// Current total redemptions used
    pub total_redemptions_used: i32,
    /// Whether this utility is currently active
    pub is_active: bool,
    /// How to claim/redeem (instructions)
    pub redemption_instructions: Option<String>,
    /// External link for redemption (calendar, form, etc.)
    pub redemption_url: Option<String>,
    /// Estimated value in BTC (for display purposes)
    pub estimated_value_btc: Option<Decimal>,
    /// When this utility expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
    /// Timestamp when this utility was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent update to this utility.
    pub updated_at: DateTime<Utc>,
}

impl fmt::Display for UtilityDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Utility({}, {}, type={}, min_tokens={})",
            self.utility_id, self.title, self.utility_type, self.min_tokens_required
        )
    }
}

impl UtilityDefinition {
    /// Check if this utility is available for redemption
    pub fn is_available(&self) -> bool {
        if !self.is_active {
            return false;
        }
        if let Some(expires_at) = self.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }
        if let Some(max_total) = self.max_total_redemptions {
            if self.total_redemptions_used >= max_total {
                return false;
            }
        }
        true
    }

    /// Check if a user with given token balance can access this utility
    pub fn can_access(&self, user_balance: Decimal) -> bool {
        self.is_available() && user_balance >= self.min_tokens_required
    }

    /// Get remaining total redemptions (None if unlimited)
    pub fn remaining_redemptions(&self) -> Option<i32> {
        self.max_total_redemptions
            .map(|max| (max - self.total_redemptions_used).max(0))
    }
}

/// Request payload for creating a new utility definition
#[derive(Debug, Deserialize)]
pub struct CreateUtilityRequest {
    /// Type of utility/benefit
    pub utility_type: UtilityType,
    /// Title of the utility
    pub title: String,
    /// Optional detailed description
    pub description: Option<String>,
    /// Minimum token balance required to redeem
    pub min_tokens_required: Decimal,
    /// Optional cap on redemptions per holder
    pub max_redemptions_per_holder: Option<i32>,
    /// Optional cap on total redemptions
    pub max_total_redemptions: Option<i32>,
    /// Instructions for redeeming the utility
    pub redemption_instructions: Option<String>,
    /// Optional URL for redemption (e.g. calendar link)
    pub redemption_url: Option<String>,
    /// Estimated value in BTC for display
    pub estimated_value_btc: Option<Decimal>,
    /// Optional expiration date
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateUtilityRequest {
    /// Validate the utility creation request fields
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.title.is_empty() {
            return Err(ValidationError("Title is required".to_string()));
        }
        if self.title.len() > 200 {
            return Err(ValidationError(
                "Title must be at most 200 characters".to_string(),
            ));
        }
        if let Some(ref desc) = self.description {
            if desc.len() > 2000 {
                return Err(ValidationError(
                    "Description must be at most 2000 characters".to_string(),
                ));
            }
        }
        if self.min_tokens_required < dec!(0) {
            return Err(ValidationError(
                "Minimum tokens required cannot be negative".to_string(),
            ));
        }
        if let Some(max) = self.max_redemptions_per_holder {
            if max <= 0 {
                return Err(ValidationError(
                    "Max redemptions per holder must be positive".to_string(),
                ));
            }
        }
        if let Some(max) = self.max_total_redemptions {
            if max <= 0 {
                return Err(ValidationError(
                    "Max total redemptions must be positive".to_string(),
                ));
            }
        }
        if let Some(ref url) = self.redemption_url {
            if !url.starts_with("https://") {
                return Err(ValidationError("Redemption URL must use HTTPS".to_string()));
            }
        }
        if let Some(expires) = self.expires_at {
            if expires < Utc::now() {
                return Err(ValidationError(
                    "Expiration date must be in the future".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Record of a utility redemption by a token holder
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UtilityRedemption {
    /// Unique identifier of this redemption record.
    pub redemption_id: Uuid,
    /// Utility that was redeemed.
    pub utility_id: Uuid,
    /// User who performed the redemption.
    pub user_id: Uuid,
    /// Token balance at time of redemption
    pub tokens_held: Decimal,
    /// Status of the redemption
    pub status: RedemptionStatus,
    /// Notes from the user or issuer
    pub notes: Option<String>,
    /// Timestamp when the redemption was requested.
    pub redeemed_at: DateTime<Utc>,
    /// Timestamp when the issuer fulfilled the redemption, if fulfilled.
    pub fulfilled_at: Option<DateTime<Utc>>,
}

/// Status of a utility redemption request
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum RedemptionStatus {
    /// Redemption requested
    #[default]
    Pending,
    /// Issuer confirmed/scheduled
    Confirmed,
    /// Utility delivered/fulfilled
    Fulfilled,
    /// Cancelled by user or issuer
    Cancelled,
    /// Expired without fulfillment
    Expired,
}

impl fmt::Display for RedemptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedemptionStatus::Pending => write!(f, "pending"),
            RedemptionStatus::Confirmed => write!(f, "confirmed"),
            RedemptionStatus::Fulfilled => write!(f, "fulfilled"),
            RedemptionStatus::Cancelled => write!(f, "cancelled"),
            RedemptionStatus::Expired => write!(f, "expired"),
        }
    }
}

/// Summary of utilities for a token (for display)
#[derive(Debug, Serialize)]
pub struct UtilitySummary {
    /// Token this summary belongs to
    pub token_id: Uuid,
    /// Total number of defined utilities
    pub total_utilities: i32,
    /// Number of currently active utilities
    pub active_utilities: i32,
    /// Full list of utility definitions
    pub utilities: Vec<UtilityDefinition>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    fn valid_create_token_request() -> CreateTokenRequest {
        CreateTokenRequest {
            symbol: "$MYTOKEN".to_string(),
            name: "My Token".to_string(),
            description: None,
            total_supply: dec!(1000),
            initial_price_btc: dec!(0.001),
            price_increment_btc: dec!(0.0001),
        }
    }

    fn make_utility_definition(
        is_active: bool,
        expires_at: Option<DateTime<Utc>>,
        max_total: Option<i32>,
        used: i32,
    ) -> UtilityDefinition {
        let now = Utc::now();
        UtilityDefinition {
            utility_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            utility_type: UtilityType::ContentAccess,
            title: "Test Utility".to_string(),
            description: None,
            min_tokens_required: dec!(10),
            max_redemptions_per_holder: None,
            max_total_redemptions: max_total,
            total_redemptions_used: used,
            is_active,
            redemption_instructions: None,
            redemption_url: None,
            estimated_value_btc: None,
            expires_at,
            created_at: now,
            updated_at: now,
        }
    }

    // ----------------------------------------------------------------
    // CreateTokenRequest::validate — success path
    // ----------------------------------------------------------------

    #[test]
    fn test_create_token_request_valid() {
        let req = valid_create_token_request();
        assert!(
            req.validate().is_ok(),
            "A well-formed request must pass validation"
        );
    }

    // ----------------------------------------------------------------
    // CreateTokenRequest::validate — rejection paths
    // ----------------------------------------------------------------

    #[test]
    fn test_create_token_empty_symbol_rejected() {
        let req = CreateTokenRequest {
            symbol: "".to_string(),
            ..valid_create_token_request()
        };
        match req.validate() {
            Err(ValidationError(msg)) => {
                assert!(msg.contains("Symbol is required"), "message was: {msg}")
            }
            Ok(()) => panic!("Expected validation error for empty symbol"),
        }
    }

    #[test]
    fn test_create_token_symbol_without_dollar_prefix_rejected() {
        let req = CreateTokenRequest {
            symbol: "MYTOKEN".to_string(),
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Symbol without '$' must be rejected");
        assert!(
            err.0.contains('$'),
            "error message should mention '$': {}",
            err.0
        );
    }

    #[test]
    fn test_create_token_symbol_too_long_rejected() {
        // 21-char symbol (including $)
        let req = CreateTokenRequest {
            symbol: "$ABCDEFGHIJKLMNOPQRST".to_string(), // 21 chars
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Overly long symbol must be rejected");
        assert!(err.0.contains("20 characters"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_symbol_lowercase_rejected() {
        let req = CreateTokenRequest {
            symbol: "$mytoken".to_string(),
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Lowercase symbol must be rejected");
        assert!(err.0.contains("uppercase"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_zero_supply_rejected() {
        let req = CreateTokenRequest {
            total_supply: dec!(0),
            ..valid_create_token_request()
        };
        let err = req.validate().expect_err("Zero supply must be rejected");
        assert!(err.0.contains("positive"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_supply_exceeds_maximum_rejected() {
        let req = CreateTokenRequest {
            total_supply: dec!(1_000_001),
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Supply beyond 1M must be rejected");
        assert!(err.0.contains("1,000,000"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_zero_price_rejected() {
        let req = CreateTokenRequest {
            initial_price_btc: dec!(0),
            ..valid_create_token_request()
        };
        let err = req.validate().expect_err("Zero price must be rejected");
        assert!(err.0.contains("positive"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_price_below_minimum_rejected() {
        let req = CreateTokenRequest {
            initial_price_btc: dec!(0.000001),
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Price below 0.00001 must be rejected");
        assert!(err.0.contains("0.00001"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_negative_price_increment_rejected() {
        let req = CreateTokenRequest {
            price_increment_btc: dec!(-0.001),
            ..valid_create_token_request()
        };
        let err = req
            .validate()
            .expect_err("Negative price increment must be rejected");
        assert!(err.0.contains("negative"), "error: {}", err.0);
    }

    #[test]
    fn test_create_token_empty_name_rejected() {
        let req = CreateTokenRequest {
            name: "".to_string(),
            ..valid_create_token_request()
        };
        let err = req.validate().expect_err("Empty name must be rejected");
        assert!(err.0.contains("Name is required"), "error: {}", err.0);
    }

    // ----------------------------------------------------------------
    // TokenMetadata::validate
    // ----------------------------------------------------------------

    #[test]
    fn test_token_metadata_empty_is_valid() {
        let meta = TokenMetadata::empty(Uuid::new_v4());
        assert!(meta.validate().is_ok(), "Empty metadata must be valid");
    }

    #[test]
    fn test_token_metadata_http_website_rejected() {
        let mut meta = TokenMetadata::empty(Uuid::new_v4());
        meta.website_url = Some("http://example.com".to_string());
        let err = meta
            .validate()
            .expect_err("HTTP website URL must be rejected");
        assert!(err.0.contains("HTTPS"), "error: {}", err.0);
    }

    #[test]
    fn test_token_metadata_invalid_github_url_rejected() {
        let mut meta = TokenMetadata::empty(Uuid::new_v4());
        meta.github_url = Some("https://example.com/repo".to_string());
        let err = meta
            .validate()
            .expect_err("Non-github.com URL must be rejected");
        assert!(err.0.contains("GitHub"), "error: {}", err.0);
    }

    #[test]
    fn test_token_metadata_valid_github_url_accepted() {
        let mut meta = TokenMetadata::empty(Uuid::new_v4());
        meta.github_url = Some("https://github.com/cool-japan/kaccy".to_string());
        assert!(meta.validate().is_ok(), "Valid GitHub URL must be accepted");
    }

    // ----------------------------------------------------------------
    // TokenMetadata::tags_list
    // ----------------------------------------------------------------

    #[test]
    fn test_tags_list_parses_comma_separated() {
        let mut meta = TokenMetadata::empty(Uuid::new_v4());
        meta.tags = Some("defi, nft, web3".to_string());
        let tags = meta.tags_list();
        assert_eq!(tags, vec!["defi", "nft", "web3"]);
    }

    #[test]
    fn test_tags_list_empty_when_no_tags() {
        let meta = TokenMetadata::empty(Uuid::new_v4());
        assert!(
            meta.tags_list().is_empty(),
            "No tags must return empty list"
        );
    }

    #[test]
    fn test_tags_list_filters_empty_segments() {
        let mut meta = TokenMetadata::empty(Uuid::new_v4());
        meta.tags = Some(",defi,,nft,".to_string());
        let tags = meta.tags_list();
        assert_eq!(tags, vec!["defi", "nft"]);
    }

    // ----------------------------------------------------------------
    // UtilityDefinition::is_available
    // ----------------------------------------------------------------

    #[test]
    fn test_utility_available_when_active_and_not_expired() {
        let utility = make_utility_definition(true, None, None, 0);
        assert!(
            utility.is_available(),
            "Active, unexpired utility must be available"
        );
    }

    #[test]
    fn test_utility_unavailable_when_inactive() {
        let utility = make_utility_definition(false, None, None, 0);
        assert!(
            !utility.is_available(),
            "Inactive utility must not be available"
        );
    }

    #[test]
    fn test_utility_unavailable_when_expired() {
        let past = Utc::now() - Duration::days(1);
        let utility = make_utility_definition(true, Some(past), None, 0);
        assert!(
            !utility.is_available(),
            "Expired utility must not be available"
        );
    }

    #[test]
    fn test_utility_unavailable_when_redemptions_exhausted() {
        let utility = make_utility_definition(true, None, Some(5), 5);
        assert!(
            !utility.is_available(),
            "Utility with all redemptions used must not be available"
        );
    }

    // ----------------------------------------------------------------
    // UtilityDefinition::can_access
    // ----------------------------------------------------------------

    #[test]
    fn test_can_access_with_sufficient_balance() {
        let utility = make_utility_definition(true, None, None, 0);
        // min_tokens_required = 10
        assert!(
            utility.can_access(dec!(10)),
            "Balance exactly at minimum must grant access"
        );
        assert!(
            utility.can_access(dec!(100)),
            "Balance above minimum must grant access"
        );
    }

    #[test]
    fn test_cannot_access_with_insufficient_balance() {
        let utility = make_utility_definition(true, None, None, 0);
        assert!(
            !utility.can_access(dec!(9)),
            "Balance below minimum must deny access"
        );
    }

    // ----------------------------------------------------------------
    // UtilityDefinition::remaining_redemptions
    // ----------------------------------------------------------------

    #[test]
    fn test_remaining_redemptions_unlimited() {
        let utility = make_utility_definition(true, None, None, 0);
        assert_eq!(
            utility.remaining_redemptions(),
            None,
            "Unlimited utility must return None for remaining"
        );
    }

    #[test]
    fn test_remaining_redemptions_limited() {
        let utility = make_utility_definition(true, None, Some(10), 3);
        assert_eq!(
            utility.remaining_redemptions(),
            Some(7),
            "Remaining must equal max minus used"
        );
    }

    #[test]
    fn test_remaining_redemptions_never_negative() {
        // Over-used (should not happen in practice, but guard exists)
        let utility = make_utility_definition(true, None, Some(5), 7);
        assert_eq!(
            utility.remaining_redemptions(),
            Some(0),
            "Remaining must be clamped to zero even if used > max"
        );
    }

    // ----------------------------------------------------------------
    // TokenStatus Display
    // ----------------------------------------------------------------

    #[test]
    fn test_token_status_display() {
        assert_eq!(TokenStatus::Active.to_string(), "active");
        assert_eq!(TokenStatus::Paused.to_string(), "paused");
        assert_eq!(TokenStatus::Closed.to_string(), "closed");
    }
}
