//! High-level AI service facade that combines all AI capabilities
//!
//! This module provides a unified interface to all AI services in the kaccy-ai crate,
//! making it easier to use multiple AI features together.
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_ai::service::AiServiceHub;
//! use kaccy_ai::llm::{LlmClient, OpenAiClient};
//! use kaccy_ai::evaluator::QualityEvaluator;
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an LLM client
//! let openai = OpenAiClient::with_default_model("your-api-key");
//! let llm_client = Arc::new(LlmClient::new(Box::new(openai)));
//!
//! // Create the AI service hub
//! let ai_service = AiServiceHub::new(llm_client);
//!
//! // Create AI components from the service
//! let evaluator = ai_service.evaluator();
//! let verifier = ai_service.verifier();
//! let fraud_detector = ai_service.fraud_detector();
//!
//! // Use the components
//! let code = "fn main() { println!(\"Hello\"); }";
//! let result = evaluator.evaluate_code(code, "rust").await?;
//!
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use crate::access_control::{AccessControlManager, AiFeature, TokenHolder};
use crate::ai_evaluator::{AiCommitmentVerifier, AiEvaluator, AiFraudDetector, EvaluatorConfig};
use crate::error::{AiError, Result};
use crate::fraud::FraudAnalysisService;
use crate::github::{GitHubClient, GitHubConfig, GitHubVerifier};
use crate::knowledge_base::KnowledgeBase;
use crate::llm::LlmClient;
use crate::oracle::{AiOracle, OracleConfig};
use crate::reputation_predictor::ReputationPredictor;
use crate::token_analyzer::TokenAnalyzer;

/// Configuration for the AI service hub
#[derive(Debug, Clone, Default)]
pub struct AiServiceConfig {
    /// Evaluator configuration
    #[allow(clippy::derivable_impls)]
    pub evaluator_config: EvaluatorConfig,
    /// Oracle configuration (if enabled)
    pub oracle_config: Option<OracleConfig>,
    /// GitHub configuration (if enabled)
    pub github_config: Option<GitHubConfig>,
    /// Enable access control
    pub enable_access_control: bool,
}

/// High-level AI service hub that combines all AI capabilities
///
/// This provides a unified interface to:
/// - Code and content evaluation
/// - Commitment verification
/// - Fraud detection
/// - GitHub integration
/// - Reputation prediction
/// - Token analysis
/// - AI Oracle for automated decisions
/// - Access control
///
/// Components are created on-demand rather than pre-instantiated for efficiency.
pub struct AiServiceHub {
    llm: Arc<LlmClient>,
    config: AiServiceConfig,
    knowledge_base: KnowledgeBase,
    access_control: Option<AccessControlManager>,
}

impl AiServiceHub {
    /// Create a new AI service hub with default configuration
    #[must_use]
    pub fn new(llm: Arc<LlmClient>) -> Self {
        Self::with_config(llm, AiServiceConfig::default())
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(llm: Arc<LlmClient>, config: AiServiceConfig) -> Self {
        let knowledge_base = KnowledgeBase::new();
        let access_control = if config.enable_access_control {
            Some(AccessControlManager::new())
        } else {
            None
        };

        Self {
            llm,
            config,
            knowledge_base,
            access_control,
        }
    }

    /// Get the evaluator configuration
    #[must_use]
    pub fn evaluator_config(&self) -> &EvaluatorConfig {
        &self.config.evaluator_config
    }

    /// Get the oracle configuration (if enabled)
    #[must_use]
    pub fn oracle_config(&self) -> Option<&OracleConfig> {
        self.config.oracle_config.as_ref()
    }

    /// Get the GitHub configuration (if enabled)
    #[must_use]
    pub fn github_config(&self) -> Option<&GitHubConfig> {
        self.config.github_config.as_ref()
    }

    /// Create an AI evaluator instance
    #[must_use]
    pub fn evaluator(&self) -> AiEvaluator {
        AiEvaluator::with_config((*self.llm).clone(), self.config.evaluator_config.clone())
    }

    /// Create an AI commitment verifier instance
    #[must_use]
    pub fn verifier(&self) -> AiCommitmentVerifier {
        AiCommitmentVerifier::new((*self.llm).clone())
    }

    /// Create an AI fraud detector instance
    #[must_use]
    pub fn fraud_detector(&self) -> AiFraudDetector {
        AiFraudDetector::new((*self.llm).clone())
    }

    /// Create a fraud analysis service instance
    #[must_use]
    pub fn fraud_service(&self) -> FraudAnalysisService {
        FraudAnalysisService::new()
    }

    /// Create a reputation predictor instance
    #[must_use]
    pub fn reputation_predictor(&self) -> ReputationPredictor {
        ReputationPredictor::new((*self.llm).clone())
    }

    /// Create a token analyzer instance
    #[must_use]
    pub fn token_analyzer(&self) -> TokenAnalyzer {
        TokenAnalyzer::new((*self.llm).clone())
    }

    /// Create an AI Oracle instance (if configured)
    pub fn oracle(&self) -> Result<AiOracle> {
        let config = self.config.oracle_config.clone().ok_or_else(|| {
            AiError::Configuration(
                "Oracle not configured. Use with_oracle() in builder.".to_string(),
            )
        })?;
        Ok(AiOracle::new(config))
    }

    /// Create a GitHub verifier instance (if configured)
    pub fn github_verifier(&self) -> Result<GitHubVerifier> {
        let github_config = self.config.github_config.clone().ok_or_else(|| {
            AiError::Configuration(
                "GitHub integration not configured. Use with_github() in builder.".to_string(),
            )
        })?;
        let github_client = GitHubClient::new(github_config)?;
        Ok(GitHubVerifier::new(github_client))
    }

    /// Check access control for a token holder
    ///
    /// Returns None if access control is not enabled
    #[must_use]
    pub fn can_access_feature(
        &self,
        holder: &TokenHolder,
        feature: AiFeature,
    ) -> Option<Result<bool>> {
        self.access_control
            .as_ref()
            .map(|ac| ac.can_access_feature(holder, feature))
    }

    /// Record feature usage for access control
    ///
    /// Returns None if access control is not enabled
    pub fn record_usage(&mut self, holder: &TokenHolder, feature: AiFeature) -> Option<Result<()>> {
        self.access_control
            .as_mut()
            .map(|ac| ac.record_usage(holder, feature))
    }

    /// Access the knowledge base
    #[must_use]
    pub fn knowledge_base(&self) -> &KnowledgeBase {
        &self.knowledge_base
    }

    /// Get mutable access to the knowledge base
    pub fn knowledge_base_mut(&mut self) -> &mut KnowledgeBase {
        &mut self.knowledge_base
    }

    /// Get direct access to the LLM client
    ///
    /// Useful for advanced use cases where you need direct LLM access.
    /// For most cases, use the component factory methods like `evaluator()`,
    /// `verifier()`, etc.
    #[must_use]
    pub fn llm_client(&self) -> &Arc<LlmClient> {
        &self.llm
    }
}

/// Builder for `AiServiceHub` with fluent configuration
pub struct AiServiceBuilder {
    llm: Arc<LlmClient>,
    config: AiServiceConfig,
}

impl AiServiceBuilder {
    /// Create a new builder with the given LLM client
    #[must_use]
    pub fn new(llm: Arc<LlmClient>) -> Self {
        Self {
            llm,
            config: AiServiceConfig::default(),
        }
    }

    /// Set evaluator configuration
    #[must_use]
    pub fn evaluator_config(mut self, config: EvaluatorConfig) -> Self {
        self.config.evaluator_config = config;
        self
    }

    /// Enable and configure the AI Oracle
    #[must_use]
    pub fn with_oracle(mut self, config: OracleConfig) -> Self {
        self.config.oracle_config = Some(config);
        self
    }

    /// Enable and configure GitHub integration
    #[must_use]
    pub fn with_github(mut self, config: GitHubConfig) -> Self {
        self.config.github_config = Some(config);
        self
    }

    /// Enable access control
    #[must_use]
    pub fn with_access_control(mut self) -> Self {
        self.config.enable_access_control = true;
        self
    }

    /// Build the AI service hub
    #[must_use]
    pub fn build(self) -> AiServiceHub {
        AiServiceHub::with_config(self.llm, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::OpenAiClient;

    #[test]
    fn test_service_builder() {
        let openai = OpenAiClient::with_default_model("test-key");
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));

        let service = AiServiceBuilder::new(llm_client)
            .with_access_control()
            .build();

        assert!(service.access_control.is_some());
        assert!(service.config.oracle_config.is_none());
        assert!(service.config.github_config.is_none());
    }

    #[test]
    fn test_service_with_oracle() {
        let openai = OpenAiClient::with_default_model("test-key");
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));

        let oracle_config = OracleConfig::default();
        let service = AiServiceBuilder::new(llm_client)
            .with_oracle(oracle_config)
            .build();

        assert!(service.config.oracle_config.is_some());
        assert!(service.oracle().is_ok());
    }

    #[test]
    fn test_service_with_github() {
        let openai = OpenAiClient::with_default_model("test-key");
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));

        let github_config = GitHubConfig::default();
        let service = AiServiceBuilder::new(llm_client)
            .with_github(github_config)
            .build();

        assert!(service.config.github_config.is_some());
        assert!(service.github_verifier().is_ok());
    }

    #[test]
    fn test_default_service() {
        let openai = OpenAiClient::with_default_model("test-key");
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));

        let service = AiServiceHub::new(llm_client);

        assert!(service.access_control.is_none());
        assert!(service.config.oracle_config.is_none());
        assert!(service.config.github_config.is_none());
    }
}
