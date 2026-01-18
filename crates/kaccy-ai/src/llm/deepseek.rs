//! `DeepSeek` LLM provider integration
//!
//! This module provides integration with `DeepSeek`'s API, supporting various `DeepSeek` models.
//! `DeepSeek` offers cost-effective AI models with competitive performance.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{AiError, Result};
use crate::llm::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, CompletionRequest, CompletionResponse,
    LlmProvider, StreamChunk, StreamResponse, StreamingChatRequest, StreamingLlmProvider,
};

/// `DeepSeek` client
#[derive(Clone)]
pub struct DeepSeekClient {
    api_key: String,
    model: String,
    client: Client,
    base_url: String,
}

impl DeepSeekClient {
    /// Create a new `DeepSeek` client with custom model
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: Client::new(),
            base_url: "https://api.deepseek.com/v1".to_string(),
        }
    }

    /// Create a new client with `DeepSeek` Chat model (default)
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "deepseek-chat")
    }

    /// Create a new client with `DeepSeek` Coder model (specialized for code)
    pub fn with_coder_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "deepseek-coder")
    }

    /// Create a new client with `DeepSeek` Reasoner model (advanced reasoning)
    pub fn with_reasoner_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "deepseek-reasoner")
    }

    /// Set the model
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    /// Get the current model
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Set custom base URL (for testing or alternative endpoints)
    pub fn set_base_url(&mut self, url: impl Into<String>) {
        self.base_url = url.into();
    }
}

#[async_trait]
impl LlmProvider for DeepSeekClient {
    fn name(&self) -> &'static str {
        "deepseek"
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Convert to chat format (DeepSeek uses chat API for completions)
        let messages = [ChatMessage {
            role: ChatRole::User,
            content: request.prompt,
        }];

        let req_body = DeepSeekChatRequest {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| DeepSeekMessage {
                    role: match m.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    }
                    .to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|t| t as usize),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("DeepSeek request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "DeepSeek API error ({status}): {error_text}"
            )));
        }

        let deepseek_response: DeepSeekChatResponse = response.json().await.map_err(|e| {
            AiError::ProviderError(format!("Failed to parse DeepSeek response: {e}"))
        })?;

        let content = deepseek_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| AiError::ProviderError("No content in DeepSeek response".to_string()))?;

        let usage = deepseek_response.usage.unwrap_or_default();

        Ok(CompletionResponse {
            text: content,
            prompt_tokens: usage.prompt_tokens as u32,
            completion_tokens: usage.completion_tokens as u32,
            total_tokens: usage.total_tokens as u32,
            finish_reason: deepseek_response
                .choices
                .first()
                .and_then(|c| c.finish_reason.clone()),
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let req_body = DeepSeekChatRequest {
            model: self.model.clone(),
            messages: request
                .messages
                .iter()
                .map(|m| DeepSeekMessage {
                    role: match m.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    }
                    .to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|t| t as usize),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("DeepSeek request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "DeepSeek API error ({status}): {error_text}"
            )));
        }

        let deepseek_response: DeepSeekChatResponse = response.json().await.map_err(|e| {
            AiError::ProviderError(format!("Failed to parse DeepSeek response: {e}"))
        })?;

        let content = deepseek_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| AiError::ProviderError("No content in DeepSeek response".to_string()))?;

        let usage = deepseek_response.usage.unwrap_or_default();

        Ok(ChatResponse {
            message: ChatMessage {
                role: ChatRole::Assistant,
                content,
            },
            prompt_tokens: usage.prompt_tokens as u32,
            completion_tokens: usage.completion_tokens as u32,
            total_tokens: usage.total_tokens as u32,
            finish_reason: deepseek_response
                .choices
                .first()
                .and_then(|c| c.finish_reason.clone()),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        // Try a minimal request to check if the API is accessible
        let test_request = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Hi".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: Some(5),
            stop: None,
            images: None,
        };

        match self.chat(test_request).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn clone_box(&self) -> Box<dyn LlmProvider> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl StreamingLlmProvider for DeepSeekClient {
    async fn chat_stream(&self, request: StreamingChatRequest) -> Result<StreamResponse> {
        let req_body = DeepSeekChatRequest {
            model: self.model.clone(),
            messages: request
                .request
                .messages
                .iter()
                .map(|m| DeepSeekMessage {
                    role: match m.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    }
                    .to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            temperature: request.request.temperature,
            max_tokens: request.request.max_tokens.map(|t| t as usize),
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("DeepSeek stream request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "DeepSeek API error ({status}): {error_text}"
            )));
        }

        // Convert byte stream to SSE stream
        let stream = response
            .bytes_stream()
            .map(move |chunk_result| {
                chunk_result
                    .map_err(|e| AiError::ProviderError(format!("Stream error: {e}")))
                    .and_then(|bytes| parse_deepseek_sse(&bytes))
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(chunk)) => Some(Ok(chunk)),
                    Ok(None) => None, // Skip empty chunks
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(Box::pin(stream))
    }
}

/// Parse `DeepSeek` SSE data
fn parse_deepseek_sse(bytes: &[u8]) -> Result<Option<StreamChunk>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| AiError::ProviderError(format!("Invalid UTF-8: {e}")))?;

    // SSE format: "data: {...}\n\n"
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return Ok(Some(StreamChunk {
                    delta: String::new(),
                    is_final: true,
                    stop_reason: Some("stop".to_string()),
                    index: 0,
                }));
            }

            // Parse JSON chunk
            if let Ok(chunk_data) = serde_json::from_str::<DeepSeekStreamChunk>(data) {
                if let Some(choice) = chunk_data.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        return Ok(Some(StreamChunk {
                            delta: content.clone(),
                            is_final: choice.finish_reason.is_some(),
                            stop_reason: choice.finish_reason.clone(),
                            index: 0,
                        }));
                    }
                }
            }
        }
    }

    Ok(None) // No valid chunk found
}

// DeepSeek API types

#[derive(Debug, Serialize)]
struct DeepSeekChatRequest {
    model: String,
    messages: Vec<DeepSeekMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DeepSeekMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChatResponse {
    #[allow(dead_code)]
    model: String,
    choices: Vec<DeepSeekChoice>,
    usage: Option<DeepSeekUsage>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoice {
    message: DeepSeekMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct DeepSeekUsage {
    #[serde(default)]
    prompt_tokens: usize,
    #[serde(default)]
    completion_tokens: usize,
    #[serde(default)]
    total_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct DeepSeekStreamChunk {
    choices: Vec<DeepSeekStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekStreamChoice {
    delta: DeepSeekDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekDelta {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model() {
        let client = DeepSeekClient::with_default_model("test-key");
        assert_eq!(client.model(), "deepseek-chat");
        assert_eq!(client.name(), "deepseek");
    }

    #[test]
    fn test_coder_model() {
        let client = DeepSeekClient::with_coder_model("test-key");
        assert_eq!(client.model(), "deepseek-coder");
    }

    #[test]
    fn test_reasoner_model() {
        let client = DeepSeekClient::with_reasoner_model("test-key");
        assert_eq!(client.model(), "deepseek-reasoner");
    }

    #[test]
    fn test_model_setter() {
        let mut client = DeepSeekClient::with_default_model("test-key");
        client.set_model("deepseek-coder");
        assert_eq!(client.model(), "deepseek-coder");
    }

    #[test]
    fn test_clone() {
        let client = DeepSeekClient::with_default_model("test-key");
        let cloned = client.clone();
        assert_eq!(client.model(), cloned.model());
        assert_eq!(client.name(), cloned.name());
    }

    #[test]
    fn test_custom_base_url() {
        let mut client = DeepSeekClient::with_default_model("test-key");
        client.set_base_url("https://custom.api.com");
        assert_eq!(client.base_url, "https://custom.api.com");
    }
}
