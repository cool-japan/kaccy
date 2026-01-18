//! Ollama LLM provider for local model execution
//!
//! Ollama allows running LLMs locally without API costs.
//! Supports models like Llama, Mistral, `CodeLlama`, etc.
//!
//! See: <https://ollama.ai/>

use super::types::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, CompletionRequest, CompletionResponse,
};
use super::{LlmProvider, StreamingLlmProvider};
use crate::error::{AiError, Result};
use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Ollama-specific request format
#[derive(Debug, Clone, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: Option<String>,
    messages: Option<Vec<OllamaMessage>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

/// Ollama message format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

/// Ollama generation options
#[derive(Debug, Clone, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

/// Ollama response format
#[derive(Debug, Clone, Deserialize)]
struct OllamaResponse {
    #[serde(default)]
    #[allow(dead_code)]
    model: String,
    #[serde(default)]
    message: Option<OllamaMessage>,
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    #[allow(dead_code)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<i32>,
    #[serde(default)]
    eval_count: Option<i32>,
}

/// Ollama model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaModelInfo {
    /// Model name
    pub name: String,
    /// Modified timestamp
    #[serde(default)]
    pub modified_at: Option<String>,
    /// Model size in bytes
    #[serde(default)]
    pub size: Option<u64>,
    /// Model digest
    #[serde(default)]
    pub digest: Option<String>,
}

/// Ollama tags response (list of models)
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelInfo>,
}

/// Ollama client for local LLM execution
#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: Client,
    temperature: f32,
}

impl OllamaClient {
    /// Create a new Ollama client
    ///
    /// # Arguments
    /// * `base_url` - Ollama server URL (default: "<http://localhost:11434>")
    /// * `model` - Model name (e.g., "llama2", "mistral", "codellama")
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client: Client::new(),
            temperature: 0.7,
        }
    }

    /// Create from environment variables
    ///
    /// Uses:
    /// - `OLLAMA_BASE_URL` (default: "<http://localhost:11434>")
    /// - `OLLAMA_MODEL` (default: "llama2")
    #[must_use]
    pub fn from_env() -> Self {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama2".to_string());

        Self::new(base_url, model)
    }

    /// Set the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set temperature for generation
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Convert `ChatMessage` to `OllamaMessage`
    fn convert_message(msg: &ChatMessage) -> OllamaMessage {
        OllamaMessage {
            role: match msg.role {
                ChatRole::System => "system".to_string(),
                ChatRole::User => "user".to_string(),
                ChatRole::Assistant => "assistant".to_string(),
            },
            content: msg.content.clone(),
        }
    }

    /// Convert messages to Ollama format
    fn convert_messages(messages: &[ChatMessage]) -> Vec<OllamaMessage> {
        messages.iter().map(Self::convert_message).collect()
    }

    /// List available models on the Ollama server
    pub async fn list_models(&self) -> Result<Vec<OllamaModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);
        let response =
            self.client.get(&url).send().await.map_err(|e| {
                AiError::ProviderError(format!("Failed to list Ollama models: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Ollama API error {status}: {error_text}"
            )));
        }

        let tags_response: OllamaTagsResponse = response.json().await.map_err(|e| {
            AiError::ParseError(format!("Failed to parse Ollama tags response: {e}"))
        })?;

        Ok(tags_response.models)
    }

    /// Check if a specific model is available
    pub async fn is_model_available(&self, model_name: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m.name == model_name))
    }

    /// Get recommended models for different use cases
    #[must_use]
    pub fn recommended_models() -> &'static [(&'static str, &'static str)] {
        &[
            (
                "llama2",
                "General purpose - balanced performance and quality",
            ),
            (
                "codellama",
                "Code generation and analysis - best for programming tasks",
            ),
            ("mistral", "Lightweight and fast - good for quick responses"),
            ("llama2:70b", "High quality - requires powerful hardware"),
            ("phi", "Very small and fast - good for simple tasks"),
            ("neural-chat", "Conversational AI - optimized for chat"),
        ]
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Convert completion to chat for consistency
        let chat_request = ChatRequest {
            messages: vec![ChatMessage::user(request.prompt)],
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stop: request.stop,
            images: None,
        };

        let chat_response = self.chat(chat_request).await?;

        Ok(CompletionResponse {
            text: chat_response.message.content,
            prompt_tokens: chat_response.prompt_tokens,
            completion_tokens: chat_response.completion_tokens,
            total_tokens: chat_response.total_tokens,
            finish_reason: chat_response.finish_reason,
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let ollama_messages = Self::convert_messages(&request.messages);

        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            prompt: None,
            messages: Some(ollama_messages),
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature.or(Some(self.temperature)),
                top_p: None,
                num_predict: request.max_tokens.map(|t| t as i32),
            }),
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Ollama chat request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Ollama API error {status}: {error_text}"
            )));
        }

        let ollama_response: OllamaResponse = response.json().await.map_err(|e| {
            AiError::ParseError(format!("Failed to parse Ollama chat response: {e}"))
        })?;

        let ollama_message = ollama_response
            .message
            .ok_or_else(|| AiError::ParseError("No message in Ollama response".to_string()))?;

        Ok(ChatResponse {
            message: ChatMessage {
                role: ChatRole::Assistant,
                content: ollama_message.content,
            },
            prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0) as u32,
            completion_tokens: ollama_response.eval_count.unwrap_or(0) as u32,
            total_tokens: (ollama_response.prompt_eval_count.unwrap_or(0)
                + ollama_response.eval_count.unwrap_or(0)) as u32,
            finish_reason: if ollama_response.done {
                Some("stop".to_string())
            } else {
                None
            },
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Ollama health check failed: {e}")))?;

        Ok(response.status().is_success())
    }

    fn clone_box(&self) -> Box<dyn LlmProvider> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl StreamingLlmProvider for OllamaClient {
    async fn chat_stream(
        &self,
        request: super::streaming::StreamingChatRequest,
    ) -> Result<super::streaming::StreamResponse> {
        let ollama_messages = Self::convert_messages(&request.request.messages);

        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            prompt: None,
            messages: Some(ollama_messages),
            stream: true,
            options: Some(OllamaOptions {
                temperature: request.request.temperature.or(Some(self.temperature)),
                top_p: None,
                num_predict: request.request.max_tokens.map(|t| t as i32),
            }),
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Ollama stream request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Ollama API error {status}: {error_text}"
            )));
        }

        let stream = response.bytes_stream().filter_map(|result| async move {
            match result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        if let Ok(ollama_resp) = serde_json::from_str::<OllamaResponse>(line) {
                            let delta = if let Some(message) = ollama_resp.message {
                                message.content
                            } else if let Some(response) = ollama_resp.response {
                                response
                            } else {
                                continue;
                            };

                            if !delta.is_empty() {
                                return Some(Ok(super::streaming::StreamChunk {
                                    delta,
                                    is_final: ollama_resp.done,
                                    stop_reason: if ollama_resp.done {
                                        Some("stop".to_string())
                                    } else {
                                        None
                                    },
                                    index: 0,
                                }));
                            }
                        }
                    }
                    None
                }
                Err(e) => Some(Err(AiError::ProviderError(format!("Stream error: {e}")))),
            }
        });

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OllamaClient::new("http://localhost:11434", "llama2");
        assert_eq!(client.base_url, "http://localhost:11434");
        assert_eq!(client.model, "llama2");
    }

    #[test]
    fn test_from_env_defaults() {
        unsafe {
            std::env::remove_var("OLLAMA_BASE_URL");
            std::env::remove_var("OLLAMA_MODEL");
        }
        let client = OllamaClient::from_env();
        assert_eq!(client.base_url, "http://localhost:11434");
        assert_eq!(client.model, "llama2");
    }

    #[test]
    fn test_with_model() {
        let client = OllamaClient::new("http://localhost:11434", "llama2").with_model("mistral");
        assert_eq!(client.model, "mistral");
    }

    #[test]
    fn test_with_temperature() {
        let client = OllamaClient::new("http://localhost:11434", "llama2").with_temperature(0.9);
        assert!((client.temperature - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_convert_message() {
        let msg = ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        };
        let ollama_msg = OllamaClient::convert_message(&msg);
        assert_eq!(ollama_msg.role, "user");
        assert_eq!(ollama_msg.content, "Hello");
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: "You are helpful".to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: "Hello".to_string(),
            },
        ];
        let ollama_messages = OllamaClient::convert_messages(&messages);
        assert_eq!(ollama_messages.len(), 2);
        assert_eq!(ollama_messages[0].role, "system");
        assert_eq!(ollama_messages[1].role, "user");
    }

    #[test]
    fn test_clone() {
        let client = OllamaClient::new("http://localhost:11434", "llama2");
        let cloned = client.clone();
        assert_eq!(client.base_url, cloned.base_url);
        assert_eq!(client.model, cloned.model);
    }
}
