//! LLM provider integrations
//!
//! This module provides a common interface for different LLM providers
//! (OpenAI, Anthropic, Google Gemini, DeepSeek) with easy provider switching, streaming, and caching support.
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_ai::llm::{LlmClient, OpenAiClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an OpenAI client
//! let openai = OpenAiClient::with_default_model("your-api-key");
//!
//! // Build a client
//! let client = LlmClient::new(Box::new(openai));
//! # Ok(())
//! # }
//! ```

pub mod anthropic;
pub mod budget;
pub mod cache;
pub mod circuit_breaker;
pub mod cost_optimizer;
pub mod deepseek;
pub mod gemini;
pub mod health;
pub mod metrics;
pub mod observability;
pub mod ollama;
pub mod openai;
pub mod rate_limiter;
pub mod retry;
pub mod streaming;
pub mod types;

pub use anthropic::AnthropicClient;
pub use budget::{AlertLevel, BudgetAlert, BudgetConfig, BudgetManager, BudgetPeriod, PeriodUsage};
pub use cache::{
    CacheInfo, CacheStats, CachedLlmClient, CachedResponse, LlmCache, LlmCacheConfig,
    RequestDeduplicator,
};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerMetrics, CircuitState,
};
pub use cost_optimizer::{
    BatchItem, BatchProcessor, CostTracker, ModelRouter, ModelTier, RoutingConfig, TaskComplexity,
};
pub use deepseek::DeepSeekClient;
pub use gemini::GeminiClient;
pub use health::{HealthCheckConfig, HealthMonitor, HealthStatus, HealthSummary, ProviderHealth};
pub use metrics::{MetricsCollector, MetricsSnapshot, OperationTimer, ProviderMetrics, TokenUsage};
pub use observability::{LlmOperation, LogLevel, PerformanceSpan};
pub use ollama::{OllamaClient, OllamaModelInfo};
pub use openai::OpenAiClient;
pub use rate_limiter::{RateLimitGuard, RateLimiter, RateLimiterConfig, TieredRateLimiter};
pub use retry::{RetryConfig, RetryExecutor, RetryPolicy, retry_with_backoff};
pub use streaming::{
    StreamAccumulator, StreamCallback, StreamChunk, StreamHandler, StreamResponse,
    StreamingChatRequest, StreamingChatResponse, StreamingLlmProvider, collect_stream,
    print_handler,
};
pub use types::*;

use async_trait::async_trait;

use crate::error::Result;

/// Common trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Send a completion request
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Send a chat completion request
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;

    /// Check if the provider is available
    async fn health_check(&self) -> Result<bool>;

    /// Clone the provider into a boxed trait object
    fn clone_box(&self) -> Box<dyn LlmProvider>;
}

/// Multi-provider client that can use different LLMs
pub struct LlmClient {
    primary: Box<dyn LlmProvider>,
    fallback: Option<Box<dyn LlmProvider>>,
}

impl Clone for LlmClient {
    fn clone(&self) -> Self {
        Self {
            primary: self.primary.clone_box(),
            fallback: self.fallback.as_ref().map(|f| f.clone_box()),
        }
    }
}

impl LlmClient {
    /// Create a new client with a primary provider
    pub fn new(primary: Box<dyn LlmProvider>) -> Self {
        Self {
            primary,
            fallback: None,
        }
    }

    /// Add a fallback provider
    pub fn with_fallback(mut self, fallback: Box<dyn LlmProvider>) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Send a completion request, using fallback if primary fails
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        match self.primary.complete(request.clone()).await {
            Ok(response) => Ok(response),
            Err(e) => {
                if let Some(ref fallback) = self.fallback {
                    tracing::warn!(
                        primary = self.primary.name(),
                        error = %e,
                        "Primary provider failed, trying fallback"
                    );
                    fallback.complete(request).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Send a chat request, using fallback if primary fails
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        match self.primary.chat(request.clone()).await {
            Ok(response) => Ok(response),
            Err(e) => {
                if let Some(ref fallback) = self.fallback {
                    tracing::warn!(
                        primary = self.primary.name(),
                        error = %e,
                        "Primary provider failed, trying fallback"
                    );
                    fallback.chat(request).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Get the primary provider name
    pub fn provider_name(&self) -> &str {
        self.primary.name()
    }
}

/// Builder for creating LLM clients from environment
pub struct LlmClientBuilder {
    openai_api_key: Option<String>,
    openai_model: Option<String>,
    anthropic_api_key: Option<String>,
    anthropic_model: Option<String>,
    gemini_api_key: Option<String>,
    gemini_model: Option<String>,
    deepseek_api_key: Option<String>,
    deepseek_model: Option<String>,
    prefer_anthropic: bool,
    prefer_gemini: bool,
    prefer_deepseek: bool,
}

impl Default for LlmClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmClientBuilder {
    /// Create a new builder with no keys or preferences configured.
    pub fn new() -> Self {
        Self {
            openai_api_key: None,
            openai_model: None,
            anthropic_api_key: None,
            anthropic_model: None,
            gemini_api_key: None,
            gemini_model: None,
            deepseek_api_key: None,
            deepseek_model: None,
            prefer_anthropic: false,
            prefer_gemini: false,
            prefer_deepseek: false,
        }
    }

    /// Set OpenAI API key
    pub fn openai_api_key(mut self, key: impl Into<String>) -> Self {
        self.openai_api_key = Some(key.into());
        self
    }

    /// Set OpenAI model
    pub fn openai_model(mut self, model: impl Into<String>) -> Self {
        self.openai_model = Some(model.into());
        self
    }

    /// Set Anthropic API key
    pub fn anthropic_api_key(mut self, key: impl Into<String>) -> Self {
        self.anthropic_api_key = Some(key.into());
        self
    }

    /// Set Anthropic model
    pub fn anthropic_model(mut self, model: impl Into<String>) -> Self {
        self.anthropic_model = Some(model.into());
        self
    }

    /// Set Gemini API key
    pub fn gemini_api_key(mut self, key: impl Into<String>) -> Self {
        self.gemini_api_key = Some(key.into());
        self
    }

    /// Set Gemini model
    pub fn gemini_model(mut self, model: impl Into<String>) -> Self {
        self.gemini_model = Some(model.into());
        self
    }

    /// Set DeepSeek API key
    pub fn deepseek_api_key(mut self, key: impl Into<String>) -> Self {
        self.deepseek_api_key = Some(key.into());
        self
    }

    /// Set DeepSeek model
    pub fn deepseek_model(mut self, model: impl Into<String>) -> Self {
        self.deepseek_model = Some(model.into());
        self
    }

    /// Prefer Anthropic as primary provider
    pub fn prefer_anthropic(mut self) -> Self {
        self.prefer_anthropic = true;
        self.prefer_gemini = false;
        self.prefer_deepseek = false;
        self
    }

    /// Prefer Gemini as primary provider
    pub fn prefer_gemini(mut self) -> Self {
        self.prefer_gemini = true;
        self.prefer_anthropic = false;
        self.prefer_deepseek = false;
        self
    }

    /// Prefer DeepSeek as primary provider
    pub fn prefer_deepseek(mut self) -> Self {
        self.prefer_deepseek = true;
        self.prefer_anthropic = false;
        self.prefer_gemini = false;
        self
    }

    /// Build from environment variables
    pub fn from_env() -> Self {
        Self::new()
            .openai_api_key(std::env::var("OPENAI_API_KEY").unwrap_or_default())
            .openai_model(
                std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4-turbo".to_string()),
            )
            .anthropic_api_key(std::env::var("ANTHROPIC_API_KEY").unwrap_or_default())
            .anthropic_model(
                std::env::var("ANTHROPIC_MODEL")
                    .unwrap_or_else(|_| "claude-3-opus-20240229".to_string()),
            )
            .gemini_api_key(std::env::var("GEMINI_API_KEY").unwrap_or_default())
            .gemini_model(
                std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-1.5-pro".to_string()),
            )
            .deepseek_api_key(std::env::var("DEEPSEEK_API_KEY").unwrap_or_default())
            .deepseek_model(
                std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string()),
            )
    }

    /// Build the LLM client
    pub fn build(self) -> Option<LlmClient> {
        use deepseek::DeepSeekClient;
        use gemini::GeminiClient;

        let openai = self.openai_api_key.filter(|k| !k.is_empty()).map(|key| {
            let model = self
                .openai_model
                .unwrap_or_else(|| "gpt-4-turbo".to_string());
            Box::new(OpenAiClient::new(key, model)) as Box<dyn LlmProvider>
        });

        let anthropic = self.anthropic_api_key.filter(|k| !k.is_empty()).map(|key| {
            let model = self
                .anthropic_model
                .unwrap_or_else(|| "claude-3-opus-20240229".to_string());
            Box::new(AnthropicClient::new(key, model)) as Box<dyn LlmProvider>
        });

        let gemini = self.gemini_api_key.filter(|k| !k.is_empty()).map(|key| {
            let model = self
                .gemini_model
                .unwrap_or_else(|| "gemini-1.5-pro".to_string());
            Box::new(GeminiClient::new(key, model)) as Box<dyn LlmProvider>
        });

        let deepseek = self.deepseek_api_key.filter(|k| !k.is_empty()).map(|key| {
            let model = self
                .deepseek_model
                .unwrap_or_else(|| "deepseek-chat".to_string());
            Box::new(DeepSeekClient::new(key, model)) as Box<dyn LlmProvider>
        });

        // Collect all available providers
        let mut providers = Vec::new();
        if let Some(o) = openai {
            providers.push(o);
        }
        if let Some(a) = anthropic {
            providers.push(a);
        }
        if let Some(g) = gemini {
            providers.push(g);
        }
        if let Some(d) = deepseek {
            providers.push(d);
        }

        if providers.is_empty() {
            return None;
        }

        // Determine primary based on preference
        let primary = if self.prefer_deepseek {
            // Try deepseek, then gemini, then anthropic, then openai
            if let Some(d) = providers.iter().find(|p| p.name() == "deepseek") {
                Some(d.clone_box())
            } else if let Some(g) = providers.iter().find(|p| p.name() == "gemini") {
                Some(g.clone_box())
            } else if let Some(a) = providers.iter().find(|p| p.name() == "anthropic") {
                Some(a.clone_box())
            } else {
                providers.first().map(|p| p.clone_box())
            }
        } else if self.prefer_gemini {
            // Try gemini, then deepseek, then anthropic, then openai
            if let Some(g) = providers.iter().find(|p| p.name() == "gemini") {
                Some(g.clone_box())
            } else if let Some(d) = providers.iter().find(|p| p.name() == "deepseek") {
                Some(d.clone_box())
            } else if let Some(a) = providers.iter().find(|p| p.name() == "anthropic") {
                Some(a.clone_box())
            } else {
                providers.first().map(|p| p.clone_box())
            }
        } else if self.prefer_anthropic {
            // Try anthropic, then gemini, then deepseek, then openai
            if let Some(a) = providers.iter().find(|p| p.name() == "anthropic") {
                Some(a.clone_box())
            } else if let Some(g) = providers.iter().find(|p| p.name() == "gemini") {
                Some(g.clone_box())
            } else if let Some(d) = providers.iter().find(|p| p.name() == "deepseek") {
                Some(d.clone_box())
            } else {
                providers.first().map(|p| p.clone_box())
            }
        } else {
            // Default: prefer openai, then anthropic, then gemini, then deepseek
            if let Some(o) = providers.iter().find(|p| p.name() == "openai") {
                Some(o.clone_box())
            } else if let Some(a) = providers.iter().find(|p| p.name() == "anthropic") {
                Some(a.clone_box())
            } else if let Some(g) = providers.iter().find(|p| p.name() == "gemini") {
                Some(g.clone_box())
            } else {
                providers.first().map(|p| p.clone_box())
            }
        };

        let primary = primary?;

        // Use first available different provider as fallback
        let fallback = providers
            .iter()
            .find(|p| p.name() != primary.name())
            .map(|p| p.clone_box());

        let mut client = LlmClient::new(primary);
        if let Some(fb) = fallback {
            client = client.with_fallback(fb);
        }

        Some(client)
    }
}
