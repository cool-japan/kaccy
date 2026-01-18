//! `OpenAI` API client
//!
//! Integration with `OpenAI`'s GPT models with streaming support.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{
    ChatRequest, ChatResponse, CompletionRequest, CompletionResponse, LlmProvider,
    streaming::{StreamChunk, StreamResponse, StreamingChatRequest, StreamingLlmProvider},
    types::{ChatMessage, ChatRole},
};
use crate::error::{AiError, Result};

const OPENAI_API_URL: &str = "https://api.openai.com/v1";

/// `OpenAI` API client
#[derive(Clone)]
pub struct OpenAiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl OpenAiClient {
    /// Create a new `OpenAI` client
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Create with default model (gpt-4-turbo)
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "gpt-4-turbo")
    }

    /// Set a different model
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl LlmProvider for OpenAiClient {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // OpenAI uses chat completions API for most modern models
        // Convert completion request to chat request
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
        let api_request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: request
                .messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: match m.role {
                        ChatRole::System => "system".to_string(),
                        ChatRole::User => "user".to_string(),
                        ChatRole::Assistant => "assistant".to_string(),
                    },
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stop: request.stop,
        };

        let response = self
            .client
            .post(format!("{OPENAI_API_URL}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("OpenAI request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "OpenAI API error ({status}): {error_text}"
            )));
        }

        let api_response: OpenAiChatResponse = response
            .json()
            .await
            .map_err(|e| AiError::ProviderError(format!("Failed to parse OpenAI response: {e}")))?;

        let choice =
            api_response.choices.into_iter().next().ok_or_else(|| {
                AiError::ProviderError("No choices in OpenAI response".to_string())
            })?;

        let role = match choice.message.role.as_str() {
            "system" => ChatRole::System,
            "user" => ChatRole::User,
            _ => ChatRole::Assistant,
        };

        Ok(ChatResponse {
            message: ChatMessage {
                role,
                content: choice.message.content,
            },
            prompt_tokens: api_response.usage.prompt_tokens,
            completion_tokens: api_response.usage.completion_tokens,
            total_tokens: api_response.usage.total_tokens,
            finish_reason: choice.finish_reason,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{OPENAI_API_URL}/models"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("OpenAI health check failed: {e}")))?;

        Ok(response.status().is_success())
    }

    fn clone_box(&self) -> Box<dyn LlmProvider> {
        Box::new(self.clone())
    }
}

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// Streaming types

#[derive(Debug, Serialize)]
struct OpenAiStreamRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<StreamChoice>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    index: u32,
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
}

#[async_trait]
impl StreamingLlmProvider for OpenAiClient {
    async fn chat_stream(&self, request: StreamingChatRequest) -> Result<StreamResponse> {
        let api_request = OpenAiStreamRequest {
            model: self.model.clone(),
            messages: request
                .request
                .messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: match m.role {
                        ChatRole::System => "system".to_string(),
                        ChatRole::User => "user".to_string(),
                        ChatRole::Assistant => "assistant".to_string(),
                    },
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: request.request.max_tokens,
            temperature: request.request.temperature,
            stop: request.request.stop,
            stream: true,
            stream_options: if request.include_usage {
                Some(StreamOptions {
                    include_usage: true,
                })
            } else {
                None
            },
        };

        let response = self
            .client
            .post(format!("{OPENAI_API_URL}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("OpenAI stream request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "OpenAI API error ({status}): {error_text}"
            )));
        }

        // Convert byte stream to SSE stream
        let stream = response
            .bytes_stream()
            .map(move |chunk_result| {
                chunk_result
                    .map_err(|e| AiError::ProviderError(format!("Stream error: {e}")))
                    .and_then(|bytes| parse_openai_sse(&bytes))
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

/// Parse `OpenAI` SSE data
fn parse_openai_sse(bytes: &[u8]) -> Result<Option<StreamChunk>> {
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
            let chunk: OpenAiStreamChunk = serde_json::from_str(data)
                .map_err(|e| AiError::ProviderError(format!("Failed to parse chunk: {e}")))?;

            if let Some(choice) = chunk.choices.first() {
                let delta = choice.delta.content.clone().unwrap_or_default();
                let is_final = choice.finish_reason.is_some();

                return Ok(Some(StreamChunk {
                    delta,
                    is_final,
                    stop_reason: choice.finish_reason.clone(),
                    index: choice.index,
                }));
            }
        }
    }

    Ok(None)
}
