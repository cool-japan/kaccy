//! Token (FOT - Future Output Token) model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Token {
    pub token_id: Uuid,
    pub issuer_user_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: Decimal,
    pub circulating_supply: Decimal,
    pub initial_price_btc: Decimal,
    pub price_increment_btc: Decimal,
    pub created_at: DateTime<Utc>,
    pub status: TokenStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum TokenStatus {
    #[default]
    Active,
    Paused,
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

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: Decimal,
    pub initial_price_btc: Decimal,
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

#[derive(Debug, Serialize)]
pub struct TokenWithIssuer {
    #[serde(flatten)]
    pub token: Token,
    pub issuer_username: String,
    pub issuer_display_name: Option<String>,
    pub issuer_reputation_score: Decimal,
    pub current_price: Decimal,
}

/// Sort options for token list
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenSortBy {
    #[default]
    CreatedAt,
    Symbol,
    Name,
    TotalSupply,
    CirculatingSupply,
    InitialPrice,
}

/// Sort direction
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Deserialize)]
pub struct TokenListQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub status: Option<TokenStatus>,
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
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, Deserialize)]
pub struct UpdateTokenMetadataRequest {
    pub cover_image_url: Option<String>,
    pub website_url: Option<String>,
    pub twitter_handle: Option<String>,
    pub discord_url: Option<String>,
    pub github_url: Option<String>,
    pub youtube_url: Option<String>,
    pub telegram_url: Option<String>,
    pub tags: Option<String>,
    pub faq_content: Option<String>,
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
    pub utility_id: Uuid,
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
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, Deserialize)]
pub struct CreateUtilityRequest {
    pub utility_type: UtilityType,
    pub title: String,
    pub description: Option<String>,
    pub min_tokens_required: Decimal,
    pub max_redemptions_per_holder: Option<i32>,
    pub max_total_redemptions: Option<i32>,
    pub redemption_instructions: Option<String>,
    pub redemption_url: Option<String>,
    pub estimated_value_btc: Option<Decimal>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateUtilityRequest {
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
    pub redemption_id: Uuid,
    pub utility_id: Uuid,
    pub user_id: Uuid,
    /// Token balance at time of redemption
    pub tokens_held: Decimal,
    /// Status of the redemption
    pub status: RedemptionStatus,
    /// Notes from the user or issuer
    pub notes: Option<String>,
    pub redeemed_at: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

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
    pub token_id: Uuid,
    pub total_utilities: i32,
    pub active_utilities: i32,
    pub utilities: Vec<UtilityDefinition>,
}
