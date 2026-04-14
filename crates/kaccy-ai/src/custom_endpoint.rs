//! Custom LLM endpoint support for per-issuer model configuration
//!
//! This module provides infrastructure for connecting to custom LLM endpoints,
//! managing endpoint registries per issuer, and personalizing AI assistance
//! for each token issuer's specific needs.
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_ai::custom_endpoint::{
//!     CustomEndpointConfig, CustomEndpointAuth, EndpointRequestFormat,
//!     CustomEndpointClient, CustomEndpointRegistry, IssuerPersonalization, ResponseStyle,
//! };
//! use uuid::Uuid;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = CustomEndpointConfig {
//!     endpoint_id: Uuid::new_v4(),
//!     endpoint_url: "https://my-model.company.com/v1/chat/completions".to_string(),
//!     auth: CustomEndpointAuth::ApiKey("secret-key".to_string()),
//!     model_name: "my-custom-model".to_string(),
//!     max_tokens: Some(2048),
//!     temperature: 0.7,
//!     timeout_secs: 30,
//!     request_format: EndpointRequestFormat::OpenAiCompatible,
//! };
//!
//! let client = CustomEndpointClient::new(config);
//! let response = client.complete("Explain smart contracts", None).await?;
//! println!("{}", response);
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

// ─── Error type ──────────────────────────────────────────────────────────────

/// Errors that can occur when using custom LLM endpoints
#[derive(Debug, thiserror::Error)]
pub enum CustomEndpointError {
    /// HTTP transport or connectivity error
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Response did not match any known format
    #[error("Invalid response format: {0}")]
    ParseError(String),

    /// Endpoint rejected the credentials
    #[error("Authentication failed")]
    AuthError,

    /// Endpoint could not be reached at all
    #[error("Endpoint unreachable: {0}")]
    Unreachable(String),

    /// The supplied configuration is invalid
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

// ─── Authentication ───────────────────────────────────────────────────────────

/// Authentication strategy for a custom endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CustomEndpointAuth {
    /// Bearer token / API key sent in the `Authorization` header
    ApiKey(String),
    /// Fully custom header name and value
    HeaderAuth {
        /// HTTP header name (e.g. `"X-Api-Key"`)
        header: String,
        /// Raw header value
        value: String,
    },
    /// No authentication required
    None,
}

// ─── Request format ───────────────────────────────────────────────────────────

/// How to format the outgoing JSON body for the endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndpointRequestFormat {
    /// OpenAI-compatible chat completion format:
    /// `{"model": …, "messages": [{"role": "user", "content": …}], "max_tokens": …}`
    OpenAiCompatible,

    /// Anthropic Messages API format:
    /// `{"model": …, "messages": [{"role": "user", "content": …}], "max_tokens": …}`
    AnthropicCompatible,

    /// Fully custom JSON template.  `{prompt}` is replaced with the user prompt.
    CustomJson(String),
}

// ─── Configuration ─────────────────────────────────────────────────────────────

/// Complete configuration for a single custom endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEndpointConfig {
    /// Unique identifier for this endpoint entry
    pub endpoint_id: Uuid,
    /// Full URL to the LLM API
    /// (e.g. `"https://my-model.company.com/v1/chat/completions"`)
    pub endpoint_url: String,
    /// Credentials / authentication strategy
    pub auth: CustomEndpointAuth,
    /// Model identifier to include in requests
    pub model_name: String,
    /// Optional upper bound on tokens generated per request
    pub max_tokens: Option<u32>,
    /// Sampling temperature (default `0.7`)
    pub temperature: f32,
    /// HTTP timeout in seconds (default `30`)
    pub timeout_secs: u64,
    /// How to serialise the request payload
    pub request_format: EndpointRequestFormat,
}

impl CustomEndpointConfig {
    /// Create a new config with sensible defaults.
    #[must_use]
    pub fn new(
        endpoint_url: impl Into<String>,
        auth: CustomEndpointAuth,
        model_name: impl Into<String>,
        request_format: EndpointRequestFormat,
    ) -> Self {
        Self {
            endpoint_id: Uuid::new_v4(),
            endpoint_url: endpoint_url.into(),
            auth,
            model_name: model_name.into(),
            max_tokens: None,
            temperature: 0.7,
            timeout_secs: 30,
            request_format,
        }
    }
}

// ─── Client ──────────────────────────────────────────────────────────────────

/// HTTP client that can send prompts to a single custom LLM endpoint
#[derive(Debug)]
pub struct CustomEndpointClient {
    /// Endpoint configuration
    pub config: CustomEndpointConfig,
    http_client: reqwest::Client,
}

impl CustomEndpointClient {
    /// Create a new client wrapping the given configuration
    pub fn new(config: CustomEndpointConfig) -> Self {
        let timeout = std::time::Duration::from_secs(config.timeout_secs);
        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            config,
            http_client,
        }
    }

    /// Build the JSON request body for the given prompt without sending it.
    ///
    /// Useful for testing and for inspecting what would be sent.
    pub fn build_request_body(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<Value, CustomEndpointError> {
        let body = match &self.config.request_format {
            EndpointRequestFormat::OpenAiCompatible => {
                let mut messages: Vec<Value> = Vec::new();
                if let Some(sys) = system {
                    messages.push(json!({"role": "system", "content": sys}));
                }
                messages.push(json!({"role": "user", "content": prompt}));

                let mut body = json!({
                    "model": self.config.model_name,
                    "messages": messages,
                    "temperature": self.config.temperature,
                });
                if let Some(max) = self.config.max_tokens {
                    body["max_tokens"] = json!(max);
                }
                body
            }
            EndpointRequestFormat::AnthropicCompatible => {
                let mut messages: Vec<Value> = Vec::new();
                messages.push(json!({"role": "user", "content": prompt}));

                let mut body = json!({
                    "model": self.config.model_name,
                    "messages": messages,
                    "max_tokens": self.config.max_tokens.unwrap_or(1024),
                    "temperature": self.config.temperature,
                });
                if let Some(sys) = system {
                    body["system"] = json!(sys);
                }
                body
            }
            EndpointRequestFormat::CustomJson(template) => {
                let rendered = template.replace("{prompt}", prompt);
                serde_json::from_str(&rendered).map_err(|e| {
                    CustomEndpointError::ConfigError(format!(
                        "Custom JSON template is not valid JSON after substitution: {e}"
                    ))
                })?
            }
        };
        Ok(body)
    }

    /// Apply authentication headers to the request builder
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.config.auth {
            CustomEndpointAuth::ApiKey(key) => {
                builder.header("Authorization", format!("Bearer {key}"))
            }
            CustomEndpointAuth::HeaderAuth { header, value } => {
                builder.header(header.as_str(), value.as_str())
            }
            CustomEndpointAuth::None => builder,
        }
    }

    /// Extract the text content from a response JSON value.
    ///
    /// Tries OpenAI format first (`choices[0].message.content`),
    /// then Anthropic format (`content[0].text`),
    /// then a plain `response` string field.
    fn extract_text(response_json: &Value) -> Result<String, CustomEndpointError> {
        // OpenAI format
        if let Some(text) = response_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
        {
            return Ok(text.to_string());
        }

        // Anthropic format
        if let Some(text) = response_json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|v| v.as_str())
        {
            return Ok(text.to_string());
        }

        // Generic "response" field fallback
        if let Some(text) = response_json.get("response").and_then(|v| v.as_str()) {
            return Ok(text.to_string());
        }

        Err(CustomEndpointError::ParseError(
            "Response contained no recognisable text field (checked OpenAI, Anthropic, and generic formats)".to_string(),
        ))
    }

    /// Send a prompt to the endpoint and return the generated text.
    ///
    /// Authentication headers are applied automatically based on `config.auth`.
    pub async fn complete(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<String, CustomEndpointError> {
        let body = self.build_request_body(prompt, system)?;

        let request = self
            .http_client
            .post(&self.config.endpoint_url)
            .header("Content-Type", "application/json");

        let request = self.apply_auth(request);
        let response = request.json(&body).send().await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(CustomEndpointError::AuthError);
        }

        if !response.status().is_success() {
            return Err(CustomEndpointError::Unreachable(format!(
                "Endpoint returned HTTP {}",
                response.status()
            )));
        }

        let response_json: Value = response.json().await?;
        Self::extract_text(&response_json)
    }

    /// Return `true` if the endpoint responds with HTTP 200 to a minimal probe.
    pub async fn health_check(&self) -> bool {
        // Send a very small prompt to verify connectivity and authentication
        match self.complete("ping", None).await {
            Ok(_) => true,
            Err(CustomEndpointError::HttpError(_) | CustomEndpointError::Unreachable(_)) => false,
            // Auth errors and parse errors still mean the endpoint is reachable
            Err(CustomEndpointError::AuthError | CustomEndpointError::ParseError(_)) => false,
            Err(CustomEndpointError::ConfigError(_)) => false,
        }
    }
}

// ─── Registry ────────────────────────────────────────────────────────────────

/// Manages multiple custom endpoint configurations, keyed by token/issuer ID
#[derive(Debug)]
pub struct CustomEndpointRegistry {
    /// Mapping from token_id (issuer ID) to its registered endpoint configurations
    endpoints: HashMap<Uuid, Vec<CustomEndpointConfig>>,
}

impl Default for CustomEndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomEndpointRegistry {
    /// Create an empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
        }
    }

    /// Register a new endpoint configuration for a token/issuer ID.
    ///
    /// Returns the `endpoint_id` of the newly registered config.
    pub fn register(&mut self, token_id: Uuid, config: CustomEndpointConfig) -> Uuid {
        let endpoint_id = config.endpoint_id;
        self.endpoints.entry(token_id).or_default().push(config);
        endpoint_id
    }

    /// Return all endpoint configurations registered for a given token/issuer ID.
    ///
    /// Returns an empty slice if no endpoints have been registered.
    #[must_use]
    pub fn get_endpoints(&self, token_id: &Uuid) -> &[CustomEndpointConfig] {
        self.endpoints.get(token_id).map_or(&[], |v| v.as_slice())
    }

    /// Remove the endpoint with the given URL from a token/issuer's registry.
    ///
    /// Returns `true` if an entry was found and removed.
    pub fn remove(&mut self, token_id: &Uuid, endpoint_url: &str) -> bool {
        if let Some(list) = self.endpoints.get_mut(token_id) {
            let before = list.len();
            list.retain(|c| c.endpoint_url != endpoint_url);
            let removed = list.len() < before;
            if list.is_empty() {
                self.endpoints.remove(token_id);
            }
            return removed;
        }
        false
    }

    /// Return the number of endpoint configurations registered for a token/issuer ID.
    #[must_use]
    pub fn endpoint_count(&self, token_id: &Uuid) -> usize {
        self.endpoints.get(token_id).map_or(0, |v| v.len())
    }
}

// ─── Personalization ──────────────────────────────────────────────────────────

/// Desired verbosity / tone style for issuer-personalised responses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponseStyle {
    /// Short, to-the-point answers
    Concise,
    /// Comprehensive answers with context
    Detailed,
    /// Accurate and precise, with technical terminology
    Technical,
    /// Warm, accessible language
    Friendly,
}

impl ResponseStyle {
    /// Return the style instruction appended to a system prompt
    fn instruction(&self) -> &'static str {
        match self {
            Self::Concise => "Keep your responses brief and to the point.",
            Self::Detailed => {
                "Provide comprehensive, well-structured answers with relevant context."
            }
            Self::Technical => {
                "Use precise technical terminology and provide accurate, in-depth explanations."
            }
            Self::Friendly => {
                "Use warm, accessible language and explain concepts in a friendly manner."
            }
        }
    }
}

/// Per-issuer personalisation settings that shape system prompts and responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerPersonalization {
    /// Token/issuer ID this personalisation applies to
    pub token_id: Uuid,
    /// Replaces the default system prompt when set
    pub system_prompt_override: Option<String>,
    /// Key facts about the issuer included in every prompt for context
    pub context_documents: Vec<String>,
    /// Preferred tone / verbosity of AI responses
    pub preferred_response_style: ResponseStyle,
}

impl IssuerPersonalization {
    /// Create a new personalisation record with no overrides
    #[must_use]
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            system_prompt_override: None,
            context_documents: Vec::new(),
            preferred_response_style: ResponseStyle::Detailed,
        }
    }

    /// Build a full system prompt by prepending context documents and appending
    /// a style instruction to `base_prompt`.
    ///
    /// If `system_prompt_override` is set it replaces `base_prompt` entirely.
    #[must_use]
    pub fn build_system_prompt(&self, base_prompt: &str) -> String {
        let effective_base = self
            .system_prompt_override
            .as_deref()
            .unwrap_or(base_prompt);

        let mut parts: Vec<String> = Vec::new();

        if !self.context_documents.is_empty() {
            parts.push("## Issuer Context\n".to_string());
            for (i, doc) in self.context_documents.iter().enumerate() {
                parts.push(format!("{}. {}", i + 1, doc));
            }
            parts.push(String::new()); // blank line separator
        }

        parts.push(effective_base.to_string());
        parts.push(String::new());
        parts.push(self.preferred_response_style.instruction().to_string());

        parts.join("\n")
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn openai_config() -> CustomEndpointConfig {
        CustomEndpointConfig {
            endpoint_id: Uuid::new_v4(),
            endpoint_url: "https://api.example.com/v1/chat/completions".to_string(),
            auth: CustomEndpointAuth::ApiKey("test-key".to_string()),
            model_name: "gpt-4".to_string(),
            max_tokens: Some(512),
            temperature: 0.7,
            timeout_secs: 30,
            request_format: EndpointRequestFormat::OpenAiCompatible,
        }
    }

    fn anthropic_config() -> CustomEndpointConfig {
        let mut cfg = openai_config();
        cfg.model_name = "claude-3-opus-20240229".to_string();
        cfg.request_format = EndpointRequestFormat::AnthropicCompatible;
        cfg
    }

    // ── 1. test_config_defaults ──────────────────────────────────────────────

    #[test]
    fn test_config_defaults() {
        let cfg = CustomEndpointConfig::new(
            "https://api.example.com/chat",
            CustomEndpointAuth::None,
            "my-model",
            EndpointRequestFormat::OpenAiCompatible,
        );

        assert_eq!(cfg.temperature, 0.7);
        assert_eq!(cfg.timeout_secs, 30);
        assert!(cfg.max_tokens.is_none());
    }

    // ── 2. test_openai_format_body ───────────────────────────────────────────

    #[test]
    fn test_openai_format_body() {
        let client = CustomEndpointClient::new(openai_config());
        let body = client
            .build_request_body("Hello world", Some("You are a helpful assistant"))
            .expect("build_request_body failed");

        assert_eq!(body["model"], "gpt-4");
        let messages = body["messages"].as_array().expect("messages must be array");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are a helpful assistant");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Hello world");
        assert_eq!(body["max_tokens"], 512);
    }

    // ── 3. test_anthropic_format_body ────────────────────────────────────────

    #[test]
    fn test_anthropic_format_body() {
        let client = CustomEndpointClient::new(anthropic_config());
        let body = client
            .build_request_body("What is a blockchain?", Some("Expert assistant"))
            .expect("build_request_body failed");

        assert_eq!(body["model"], "claude-3-opus-20240229");
        // system is a top-level key in Anthropic format
        assert_eq!(body["system"], "Expert assistant");
        let messages = body["messages"].as_array().expect("messages must be array");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "What is a blockchain?");
        // max_tokens must be present
        assert!(body.get("max_tokens").is_some());
    }

    // ── 4. test_custom_format_body ───────────────────────────────────────────

    #[test]
    fn test_custom_format_body() {
        let template = r#"{"input": "{prompt}", "task": "summarise"}"#.to_string();
        let mut cfg = openai_config();
        cfg.request_format = EndpointRequestFormat::CustomJson(template);
        let client = CustomEndpointClient::new(cfg);

        let body = client
            .build_request_body("Summarise this document", None)
            .expect("build_request_body failed");

        assert_eq!(body["input"], "Summarise this document");
        assert_eq!(body["task"], "summarise");
    }

    // ── 5. test_registry_register_and_get ────────────────────────────────────

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = CustomEndpointRegistry::new();
        let token_id = Uuid::new_v4();
        let cfg = openai_config();
        let url = cfg.endpoint_url.clone();

        let returned_id = registry.register(token_id, cfg);

        let endpoints = registry.get_endpoints(&token_id);
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].endpoint_url, url);
        assert_eq!(endpoints[0].endpoint_id, returned_id);
    }

    // ── 6. test_registry_remove ──────────────────────────────────────────────

    #[test]
    fn test_registry_remove() {
        let mut registry = CustomEndpointRegistry::new();
        let token_id = Uuid::new_v4();
        let cfg = openai_config();
        let url = cfg.endpoint_url.clone();

        registry.register(token_id, cfg);
        assert_eq!(registry.endpoint_count(&token_id), 1);

        let removed = registry.remove(&token_id, &url);
        assert!(removed);
        assert_eq!(registry.endpoint_count(&token_id), 0);

        // Second removal should return false
        let removed_again = registry.remove(&token_id, &url);
        assert!(!removed_again);
    }

    // ── 7. test_registry_endpoint_count ──────────────────────────────────────

    #[test]
    fn test_registry_endpoint_count() {
        let mut registry = CustomEndpointRegistry::new();
        let token_id = Uuid::new_v4();

        assert_eq!(registry.endpoint_count(&token_id), 0);

        let mut cfg1 = openai_config();
        cfg1.endpoint_url = "https://endpoint-one.example.com/chat".to_string();
        registry.register(token_id, cfg1);
        assert_eq!(registry.endpoint_count(&token_id), 1);

        let mut cfg2 = openai_config();
        cfg2.endpoint_url = "https://endpoint-two.example.com/chat".to_string();
        registry.register(token_id, cfg2);
        assert_eq!(registry.endpoint_count(&token_id), 2);

        // Different token_id should be independent
        let other_id = Uuid::new_v4();
        assert_eq!(registry.endpoint_count(&other_id), 0);
    }

    // ── 8. test_personalization_build_system_prompt ───────────────────────────

    #[test]
    fn test_personalization_build_system_prompt() {
        let token_id = Uuid::new_v4();
        let mut personalization = IssuerPersonalization::new(token_id);
        personalization.context_documents = vec![
            "We are a DeFi protocol focused on lending.".to_string(),
            "Our token is KAC.".to_string(),
        ];
        personalization.preferred_response_style = ResponseStyle::Technical;

        let prompt = personalization.build_system_prompt("You are a helpful AI assistant.");
        assert!(prompt.contains("DeFi protocol"));
        assert!(prompt.contains("KAC"));
        assert!(prompt.contains("You are a helpful AI assistant."));
        assert!(prompt.contains("technical terminology"));
    }

    #[test]
    fn test_personalization_system_prompt_override() {
        let token_id = Uuid::new_v4();
        let mut personalization = IssuerPersonalization::new(token_id);
        personalization.system_prompt_override =
            Some("You are an expert in DeFi lending protocols.".to_string());
        personalization.preferred_response_style = ResponseStyle::Concise;

        let prompt = personalization.build_system_prompt("You are a helpful AI assistant.");
        // Override replaces the base prompt
        assert!(prompt.contains("DeFi lending protocols"));
        assert!(!prompt.contains("helpful AI assistant"));
        // Style instruction is still appended
        assert!(prompt.contains("brief and to the point"));
    }
}
