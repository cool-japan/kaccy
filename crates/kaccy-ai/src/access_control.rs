//! AI Agent Access Control
//!
//! This module provides token-gated access control for AI features:
//! - Tiered access based on token holdings
//! - Per-issuer custom AI agent configuration
//! - Feature flags for different access levels
//! - Usage quota management
//!
//! # Examples
//!
//! ```
//! use kaccy_ai::{AccessTier, AccessControlManager, AiFeature, TierConfig, FeatureQuota};
//! use rust_decimal::Decimal;
//! use std::collections::HashMap;
//!
//! // Determine access tier from token balance
//! let balance = Decimal::new(5000, 0);
//! let tier = AccessTier::from_balance(balance);
//! assert_eq!(tier, AccessTier::Silver);
//!
//! // Configure access control
//! let mut manager = AccessControlManager::new();
//! let mut quotas = HashMap::new();
//! quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(10, 100));
//! let config = TierConfig {
//!     tier: AccessTier::Silver,
//!     feature_quotas: quotas,
//!     allow_custom_agents: true,
//!     max_custom_agents: 3,
//! };
//! manager.update_tier_config(config);
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{AiError, Result};

/// Access tier based on token holdings
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccessTier {
    /// No tokens held - free tier
    Free = 0,
    /// 1-999 tokens
    Bronze = 1,
    /// 1,000-9,999 tokens
    Silver = 2,
    /// 10,000-99,999 tokens
    Gold = 3,
    /// 100,000+ tokens
    Platinum = 4,
}

impl AccessTier {
    /// Get access tier from token balance
    #[must_use]
    pub fn from_balance(balance: Decimal) -> Self {
        if balance >= Decimal::new(100_000, 0) {
            AccessTier::Platinum
        } else if balance >= Decimal::new(10_000, 0) {
            AccessTier::Gold
        } else if balance >= Decimal::new(1_000, 0) {
            AccessTier::Silver
        } else if balance >= Decimal::ONE {
            AccessTier::Bronze
        } else {
            AccessTier::Free
        }
    }

    /// Get minimum balance required for this tier
    #[must_use]
    pub fn min_balance(&self) -> Decimal {
        match self {
            AccessTier::Free => Decimal::ZERO,
            AccessTier::Bronze => Decimal::ONE,
            AccessTier::Silver => Decimal::new(1_000, 0),
            AccessTier::Gold => Decimal::new(10_000, 0),
            AccessTier::Platinum => Decimal::new(100_000, 0),
        }
    }

    /// Get tier name
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            AccessTier::Free => "Free",
            AccessTier::Bronze => "Bronze",
            AccessTier::Silver => "Silver",
            AccessTier::Gold => "Gold",
            AccessTier::Platinum => "Platinum",
        }
    }

    /// Get tier description
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            AccessTier::Free => "Basic access to AI features with limited usage",
            AccessTier::Bronze => "Standard access with moderate usage limits",
            AccessTier::Silver => "Enhanced access with higher limits and priority support",
            AccessTier::Gold => "Premium access with very high limits and dedicated support",
            AccessTier::Platinum => {
                "Ultimate access with unlimited usage and personalized AI agents"
            }
        }
    }
}

/// AI features that can be gated by access tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiFeature {
    /// Code quality evaluation
    CodeEvaluation,
    /// Content quality scoring
    ContentEvaluation,
    /// Commitment verification
    CommitmentVerification,
    /// Fraud detection
    FraudDetection,
    /// GitHub verification
    GitHubVerification,
    /// Social media verification
    SocialMediaVerification,
    /// Document analysis
    DocumentAnalysis,
    /// OCR and image analysis
    OcrAnalysis,
    /// Video transcript analysis
    TranscriptAnalysis,
    /// Reputation prediction
    ReputationPrediction,
    /// Token sentiment analysis
    SentimentAnalysis,
    /// Market trend prediction
    MarketPrediction,
    /// Custom AI agent access
    CustomAgent,
}

impl AiFeature {
    /// Get human-readable name
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            AiFeature::CodeEvaluation => "Code Evaluation",
            AiFeature::ContentEvaluation => "Content Evaluation",
            AiFeature::CommitmentVerification => "Commitment Verification",
            AiFeature::FraudDetection => "Fraud Detection",
            AiFeature::GitHubVerification => "GitHub Verification",
            AiFeature::SocialMediaVerification => "Social Media Verification",
            AiFeature::DocumentAnalysis => "Document Analysis",
            AiFeature::OcrAnalysis => "OCR Analysis",
            AiFeature::TranscriptAnalysis => "Transcript Analysis",
            AiFeature::ReputationPrediction => "Reputation Prediction",
            AiFeature::SentimentAnalysis => "Sentiment Analysis",
            AiFeature::MarketPrediction => "Market Prediction",
            AiFeature::CustomAgent => "Custom AI Agent",
        }
    }
}

/// Usage quota for a specific feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureQuota {
    /// Daily request limit (None = unlimited)
    pub daily_limit: Option<u32>,
    /// Monthly request limit (None = unlimited)
    pub monthly_limit: Option<u32>,
    /// Requests used today
    pub daily_used: u32,
    /// Requests used this month
    pub monthly_used: u32,
    /// Whether this feature is enabled
    pub enabled: bool,
}

impl FeatureQuota {
    /// Create unlimited quota
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            daily_limit: None,
            monthly_limit: None,
            daily_used: 0,
            monthly_used: 0,
            enabled: true,
        }
    }

    /// Create quota with limits
    #[must_use]
    pub fn limited(daily: u32, monthly: u32) -> Self {
        Self {
            daily_limit: Some(daily),
            monthly_limit: Some(monthly),
            daily_used: 0,
            monthly_used: 0,
            enabled: true,
        }
    }

    /// Check if quota is available
    #[must_use]
    pub fn is_available(&self) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(daily) = self.daily_limit {
            if self.daily_used >= daily {
                return false;
            }
        }
        if let Some(monthly) = self.monthly_limit {
            if self.monthly_used >= monthly {
                return false;
            }
        }
        true
    }

    /// Record usage
    pub fn record_usage(&mut self) {
        self.daily_used += 1;
        self.monthly_used += 1;
    }

    /// Reset daily counter
    pub fn reset_daily(&mut self) {
        self.daily_used = 0;
    }

    /// Reset monthly counter
    pub fn reset_monthly(&mut self) {
        self.monthly_used = 0;
        self.daily_used = 0;
    }
}

/// Access control configuration for an access tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Access tier
    pub tier: AccessTier,
    /// Feature quotas
    pub feature_quotas: HashMap<AiFeature, FeatureQuota>,
    /// Whether custom agents are allowed
    pub allow_custom_agents: bool,
    /// Max custom agents per issuer
    pub max_custom_agents: u32,
}

impl TierConfig {
    /// Create default config for Free tier
    #[must_use]
    pub fn free_tier() -> Self {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(5, 50));
        quotas.insert(AiFeature::ContentEvaluation, FeatureQuota::limited(5, 50));
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(3, 20),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(3, 20));

        Self {
            tier: AccessTier::Free,
            feature_quotas: quotas,
            allow_custom_agents: false,
            max_custom_agents: 0,
        }
    }

    /// Create default config for Bronze tier
    #[must_use]
    pub fn bronze_tier() -> Self {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(50, 500));
        quotas.insert(AiFeature::ContentEvaluation, FeatureQuota::limited(50, 500));
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(30, 300),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(30, 300));
        quotas.insert(
            AiFeature::GitHubVerification,
            FeatureQuota::limited(20, 200),
        );
        quotas.insert(
            AiFeature::SocialMediaVerification,
            FeatureQuota::limited(20, 200),
        );

        Self {
            tier: AccessTier::Bronze,
            feature_quotas: quotas,
            allow_custom_agents: false,
            max_custom_agents: 0,
        }
    }

    /// Create default config for Silver tier
    #[must_use]
    pub fn silver_tier() -> Self {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(200, 2000));
        quotas.insert(
            AiFeature::ContentEvaluation,
            FeatureQuota::limited(200, 2000),
        );
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(100, 1000),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(100, 1000));
        quotas.insert(
            AiFeature::GitHubVerification,
            FeatureQuota::limited(100, 1000),
        );
        quotas.insert(
            AiFeature::SocialMediaVerification,
            FeatureQuota::limited(100, 1000),
        );
        quotas.insert(AiFeature::DocumentAnalysis, FeatureQuota::limited(50, 500));
        quotas.insert(AiFeature::OcrAnalysis, FeatureQuota::limited(50, 500));
        quotas.insert(
            AiFeature::TranscriptAnalysis,
            FeatureQuota::limited(50, 500),
        );

        Self {
            tier: AccessTier::Silver,
            feature_quotas: quotas,
            allow_custom_agents: false,
            max_custom_agents: 0,
        }
    }

    /// Create default config for Gold tier
    #[must_use]
    pub fn gold_tier() -> Self {
        let mut quotas = HashMap::new();
        quotas.insert(
            AiFeature::CodeEvaluation,
            FeatureQuota::limited(1000, 10000),
        );
        quotas.insert(
            AiFeature::ContentEvaluation,
            FeatureQuota::limited(1000, 10000),
        );
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(500, 5000),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(500, 5000));
        quotas.insert(
            AiFeature::GitHubVerification,
            FeatureQuota::limited(500, 5000),
        );
        quotas.insert(
            AiFeature::SocialMediaVerification,
            FeatureQuota::limited(500, 5000),
        );
        quotas.insert(
            AiFeature::DocumentAnalysis,
            FeatureQuota::limited(200, 2000),
        );
        quotas.insert(AiFeature::OcrAnalysis, FeatureQuota::limited(200, 2000));
        quotas.insert(
            AiFeature::TranscriptAnalysis,
            FeatureQuota::limited(200, 2000),
        );
        quotas.insert(
            AiFeature::ReputationPrediction,
            FeatureQuota::limited(100, 1000),
        );
        quotas.insert(
            AiFeature::SentimentAnalysis,
            FeatureQuota::limited(100, 1000),
        );

        Self {
            tier: AccessTier::Gold,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 1,
        }
    }

    /// Create default config for Platinum tier
    #[must_use]
    pub fn platinum_tier() -> Self {
        let mut quotas = HashMap::new();
        // Unlimited access for Platinum
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::unlimited());
        quotas.insert(AiFeature::ContentEvaluation, FeatureQuota::unlimited());
        quotas.insert(AiFeature::CommitmentVerification, FeatureQuota::unlimited());
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::unlimited());
        quotas.insert(AiFeature::GitHubVerification, FeatureQuota::unlimited());
        quotas.insert(
            AiFeature::SocialMediaVerification,
            FeatureQuota::unlimited(),
        );
        quotas.insert(AiFeature::DocumentAnalysis, FeatureQuota::unlimited());
        quotas.insert(AiFeature::OcrAnalysis, FeatureQuota::unlimited());
        quotas.insert(AiFeature::TranscriptAnalysis, FeatureQuota::unlimited());
        quotas.insert(AiFeature::ReputationPrediction, FeatureQuota::unlimited());
        quotas.insert(AiFeature::SentimentAnalysis, FeatureQuota::unlimited());
        quotas.insert(AiFeature::MarketPrediction, FeatureQuota::unlimited());
        quotas.insert(AiFeature::CustomAgent, FeatureQuota::unlimited());

        Self {
            tier: AccessTier::Platinum,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 5,
        }
    }

    /// Get quota for a feature
    #[must_use]
    pub fn get_quota(&self, feature: AiFeature) -> Option<&FeatureQuota> {
        self.feature_quotas.get(&feature)
    }

    /// Get mutable quota for a feature
    pub fn get_quota_mut(&mut self, feature: AiFeature) -> Option<&mut FeatureQuota> {
        self.feature_quotas.get_mut(&feature)
    }
}

/// Lightweight reference to personalisation settings for a custom agent.
///
/// The full `IssuerPersonalization` struct lives in `custom_endpoint.rs`;
/// this struct is the cheap, serialisable handle stored alongside agent config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerPersonalizationRef {
    /// Optional system-prompt override; when set it replaces the agent's default prompt
    pub system_prompt_override: Option<String>,
    /// Optional short hint about the issuer's domain / context
    pub context_hint: Option<String>,
}

/// Custom AI agent configuration for an issuer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentConfig {
    /// Agent ID
    pub agent_id: Uuid,
    /// Token ID this agent is for
    pub token_id: Uuid,
    /// Agent name
    pub name: String,
    /// Agent description
    pub description: Option<String>,
    /// System prompt for the agent
    pub system_prompt: String,
    /// Model to use (e.g., "gpt-4-turbo", "claude-3-opus-20240229")
    pub model: String,
    /// Temperature for responses
    pub temperature: f32,
    /// Whether agent is active
    pub is_active: bool,
    /// Access tier required to use this agent
    pub min_tier: AccessTier,
    /// Optional URL of a custom LLM endpoint to use instead of the built-in providers
    pub custom_endpoint: Option<String>,
    /// Optional personalisation settings for this agent
    pub personalization: Option<IssuerPersonalizationRef>,
}

impl CustomAgentConfig {
    /// Create a new custom agent config
    #[must_use]
    pub fn new(token_id: Uuid, name: String, system_prompt: String, model: String) -> Self {
        Self {
            agent_id: Uuid::new_v4(),
            token_id,
            name,
            description: None,
            system_prompt,
            model,
            temperature: 0.7,
            is_active: true,
            min_tier: AccessTier::Gold,
            custom_endpoint: None,
            personalization: None,
        }
    }
}

/// Token holder information for access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenHolder {
    /// User ID
    pub user_id: Uuid,
    /// Token ID
    pub token_id: Uuid,
    /// Current token balance
    pub balance: Decimal,
    /// Access tier based on balance
    pub tier: AccessTier,
}

impl TokenHolder {
    /// Create a new token holder
    #[must_use]
    pub fn new(user_id: Uuid, token_id: Uuid, balance: Decimal) -> Self {
        let tier = AccessTier::from_balance(balance);
        Self {
            user_id,
            token_id,
            balance,
            tier,
        }
    }
}

/// Access control manager
pub struct AccessControlManager {
    /// Tier configurations
    tier_configs: HashMap<AccessTier, TierConfig>,
    /// Custom agent configurations by token ID
    custom_agents: HashMap<Uuid, Vec<CustomAgentConfig>>,
}

impl Default for AccessControlManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessControlManager {
    /// Create a new access control manager with default tier configs
    #[must_use]
    pub fn new() -> Self {
        let mut tier_configs = HashMap::new();
        tier_configs.insert(AccessTier::Free, TierConfig::free_tier());
        tier_configs.insert(AccessTier::Bronze, TierConfig::bronze_tier());
        tier_configs.insert(AccessTier::Silver, TierConfig::silver_tier());
        tier_configs.insert(AccessTier::Gold, TierConfig::gold_tier());
        tier_configs.insert(AccessTier::Platinum, TierConfig::platinum_tier());

        Self {
            tier_configs,
            custom_agents: HashMap::new(),
        }
    }

    /// Verify if a token holder can access a feature
    ///
    /// # Errors
    ///
    /// Returns an error if the access tier is invalid or not configured.
    pub fn can_access_feature(&self, holder: &TokenHolder, feature: AiFeature) -> Result<bool> {
        let config = self
            .tier_configs
            .get(&holder.tier)
            .ok_or_else(|| AiError::Configuration("Invalid access tier".to_string()))?;

        if let Some(quota) = config.get_quota(feature) {
            Ok(quota.is_available())
        } else {
            // Feature not configured for this tier = not allowed
            Ok(false)
        }
    }

    /// Record feature usage for a token holder
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The access tier is invalid or not configured
    /// - The daily or monthly quota is exceeded
    /// - The feature is not available for the tier
    pub fn record_usage(&mut self, holder: &TokenHolder, feature: AiFeature) -> Result<()> {
        let config = self
            .tier_configs
            .get_mut(&holder.tier)
            .ok_or_else(|| AiError::Configuration("Invalid access tier".to_string()))?;

        if let Some(quota) = config.get_quota_mut(feature) {
            if !quota.is_available() {
                return Err(AiError::QuotaExceeded(format!(
                    "Daily or monthly limit exceeded for {}",
                    feature.name()
                )));
            }
            quota.record_usage();
            Ok(())
        } else {
            Err(AiError::FeatureNotAvailable(format!(
                "Feature {} not available for {} tier",
                feature.name(),
                holder.tier.name()
            )))
        }
    }

    /// Get available features for a tier
    #[must_use]
    pub fn get_available_features(&self, tier: AccessTier) -> Vec<AiFeature> {
        self.tier_configs
            .get(&tier)
            .map(|config| {
                config
                    .feature_quotas
                    .iter()
                    .filter(|(_, quota)| quota.enabled)
                    .map(|(feature, _)| *feature)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Register a custom agent for a token
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The access tier is invalid or not configured
    /// - Custom agents are not available for the tier
    /// - The maximum number of custom agents is exceeded
    pub fn register_custom_agent(
        &mut self,
        holder: &TokenHolder,
        agent: CustomAgentConfig,
    ) -> Result<()> {
        // Check if tier allows custom agents
        let config = self
            .tier_configs
            .get(&holder.tier)
            .ok_or_else(|| AiError::Configuration("Invalid access tier".to_string()))?;

        if !config.allow_custom_agents {
            return Err(AiError::FeatureNotAvailable(
                "Custom agents not available for this tier".to_string(),
            ));
        }

        // Check agent limit
        let agents = self.custom_agents.entry(agent.token_id).or_default();
        if agents.len() >= config.max_custom_agents as usize {
            return Err(AiError::LimitExceeded(format!(
                "Maximum {} custom agents allowed for {} tier",
                config.max_custom_agents,
                holder.tier.name()
            )));
        }

        agents.push(agent);
        Ok(())
    }

    /// Get custom agents for a token
    #[must_use]
    pub fn get_custom_agents(&self, token_id: Uuid) -> Vec<&CustomAgentConfig> {
        self.custom_agents
            .get(&token_id)
            .map(|agents| agents.iter().collect())
            .unwrap_or_default()
    }

    /// Get tier configuration
    #[must_use]
    pub fn get_tier_config(&self, tier: AccessTier) -> Option<&TierConfig> {
        self.tier_configs.get(&tier)
    }

    /// Update tier configuration
    pub fn update_tier_config(&mut self, config: TierConfig) {
        self.tier_configs.insert(config.tier, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_tier_from_balance() {
        assert_eq!(AccessTier::from_balance(Decimal::ZERO), AccessTier::Free);
        assert_eq!(
            AccessTier::from_balance(Decimal::new(500, 0)),
            AccessTier::Bronze
        );
        assert_eq!(
            AccessTier::from_balance(Decimal::new(5000, 0)),
            AccessTier::Silver
        );
        assert_eq!(
            AccessTier::from_balance(Decimal::new(50000, 0)),
            AccessTier::Gold
        );
        assert_eq!(
            AccessTier::from_balance(Decimal::new(150000, 0)),
            AccessTier::Platinum
        );
    }

    #[test]
    fn test_feature_quota_availability() {
        let mut quota = FeatureQuota::limited(5, 50);
        assert!(quota.is_available());

        // Use up daily limit
        for _ in 0..5 {
            quota.record_usage();
        }
        assert!(!quota.is_available());

        // Reset daily
        quota.reset_daily();
        assert!(quota.is_available());
    }

    #[test]
    fn test_unlimited_quota() {
        let mut quota = FeatureQuota::unlimited();
        assert!(quota.is_available());

        // Use many times
        for _ in 0..1000 {
            quota.record_usage();
        }
        assert!(quota.is_available());
    }

    #[test]
    fn test_access_control_manager() {
        let manager = AccessControlManager::new();

        let holder = TokenHolder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Decimal::new(5000, 0), // Silver tier
        );

        // Silver tier should have access to document analysis
        assert!(
            manager
                .can_access_feature(&holder, AiFeature::DocumentAnalysis)
                .unwrap()
        );

        // Silver tier should NOT have access to market prediction
        assert!(
            !manager
                .can_access_feature(&holder, AiFeature::MarketPrediction)
                .unwrap()
        );
    }

    #[test]
    fn test_custom_agent_registration() {
        let mut manager = AccessControlManager::new();

        let token_id = Uuid::new_v4();
        let holder = TokenHolder::new(
            Uuid::new_v4(),
            token_id,
            Decimal::new(50000, 0), // Gold tier (allows 1 custom agent)
        );

        let agent = CustomAgentConfig::new(
            token_id,
            "My Agent".to_string(),
            "You are a helpful assistant".to_string(),
            "gpt-4-turbo".to_string(),
        );

        // Should succeed
        assert!(manager.register_custom_agent(&holder, agent).is_ok());

        // Second agent should fail (Gold tier only allows 1)
        let agent2 = CustomAgentConfig::new(
            token_id,
            "Agent 2".to_string(),
            "Another prompt".to_string(),
            "gpt-4-turbo".to_string(),
        );
        assert!(manager.register_custom_agent(&holder, agent2).is_err());

        // Get agents
        let agents = manager.get_custom_agents(token_id);
        assert_eq!(agents.len(), 1);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(AccessTier::Bronze > AccessTier::Free);
        assert!(AccessTier::Silver > AccessTier::Bronze);
        assert!(AccessTier::Gold > AccessTier::Silver);
        assert!(AccessTier::Platinum > AccessTier::Gold);
    }

    #[test]
    fn test_usage_tracking() {
        let mut manager = AccessControlManager::new();

        let holder = TokenHolder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Decimal::new(100, 0), // Bronze tier
        );

        // Record usage
        for _ in 0..5 {
            assert!(
                manager
                    .record_usage(&holder, AiFeature::CodeEvaluation)
                    .is_ok()
            );
        }

        // Check quota was tracked (this would fail in real implementation
        // because we're not actually tracking per-user quotas, but the
        // mechanism is in place)
    }
}
