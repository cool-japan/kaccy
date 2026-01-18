//! Anthropic API client
//!
//! Integration with Anthropic's Claude models with streaming support.

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

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Claude API client
#[derive(Clone)]
pub struct AnthropicClient {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    /// Create a new Anthropic client
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Create with default model (claude-3-opus)
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "claude-3-opus-20240229")
    }

    /// Create with Claude 3.5 Sonnet
    pub fn with_sonnet(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "claude-3-5-sonnet-20241022")
    }

    /// Set a different model
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl LlmProvider for AnthropicClient {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Convert to chat request
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
        // Extract system message if present
        let (system_message, messages): (Option<String>, Vec<_>) = {
            let mut system = None;
            let mut msgs = Vec::new();

            for msg in request.messages {
                if msg.role == ChatRole::System {
                    system = Some(msg.content);
                } else {
                    msgs.push(AnthropicMessage {
                        role: match msg.role {
                            ChatRole::User => "user".to_string(),
                            ChatRole::Assistant => "assistant".to_string(),
                            ChatRole::System => continue, // Skip, handled above
                        },
                        content: msg.content,
                    });
                }
            }

            (system, msgs)
        };

        let api_request = AnthropicMessageRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(1024),
            system: system_message,
            messages,
            temperature: request.temperature,
            stop_sequences: request.stop,
        };

        let response = self
            .client
            .post(format!("{ANTHROPIC_API_URL}/messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Anthropic request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Anthropic API error ({status}): {error_text}"
            )));
        }

        let api_response: AnthropicMessageResponse = response.json().await.map_err(|e| {
            AiError::ProviderError(format!("Failed to parse Anthropic response: {e}"))
        })?;

        // Combine all text content blocks
        let content = api_response
            .content
            .iter()
            .filter_map(|block| {
                if block.content_type == "text" {
                    block.text.clone()
                } else {
                    None
                }
            })
            .collect::<String>();

        Ok(ChatResponse {
            message: ChatMessage {
                role: ChatRole::Assistant,
                content,
            },
            prompt_tokens: api_response.usage.input_tokens,
            completion_tokens: api_response.usage.output_tokens,
            total_tokens: api_response.usage.input_tokens + api_response.usage.output_tokens,
            finish_reason: Some(api_response.stop_reason.unwrap_or_default()),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        // Send a minimal message to verify API key works
        let request = AnthropicMessageRequest {
            model: self.model.clone(),
            max_tokens: 1,
            system: None,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            }],
            temperature: None,
            stop_sequences: None,
        };

        let response = self
            .client
            .post(format!("{ANTHROPIC_API_URL}/messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Anthropic health check failed: {e}")))?;

        Ok(response.status().is_success())
    }

    fn clone_box(&self) -> Box<dyn LlmProvider> {
        Box::new(self.clone())
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicMessageRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// Streaming types

#[derive(Debug, Serialize)]
struct AnthropicStreamRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum AnthropicStreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: u32,
        content_block: ContentBlockData,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<DeltaUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: ErrorData,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageStartData {
    id: String,
    model: String,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ContentBlockData {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ContentDelta {
    TextDelta { text: String },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageDeltaData {
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeltaUsage {
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ErrorData {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

#[async_trait]
impl StreamingLlmProvider for AnthropicClient {
    async fn chat_stream(&self, request: StreamingChatRequest) -> Result<StreamResponse> {
        // Extract system message if present
        let (system_message, messages): (Option<String>, Vec<_>) = {
            let mut system = None;
            let mut msgs = Vec::new();

            for msg in &request.request.messages {
                if msg.role == ChatRole::System {
                    system = Some(msg.content.clone());
                } else {
                    msgs.push(AnthropicMessage {
                        role: match msg.role {
                            ChatRole::User => "user".to_string(),
                            ChatRole::Assistant => "assistant".to_string(),
                            ChatRole::System => continue,
                        },
                        content: msg.content.clone(),
                    });
                }
            }

            (system, msgs)
        };

        let api_request = AnthropicStreamRequest {
            model: self.model.clone(),
            max_tokens: request.request.max_tokens.unwrap_or(1024),
            system: system_message,
            messages,
            temperature: request.request.temperature,
            stop_sequences: request.request.stop,
            stream: true,
        };

        let response = self
            .client
            .post(format!("{ANTHROPIC_API_URL}/messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Anthropic stream request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Anthropic API error ({status}): {error_text}"
            )));
        }

        // Convert byte stream to SSE stream
        let stream = response
            .bytes_stream()
            .map(move |chunk_result| {
                chunk_result
                    .map_err(|e| AiError::ProviderError(format!("Stream error: {e}")))
                    .and_then(|bytes| parse_anthropic_sse(&bytes))
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(chunk)) => Some(Ok(chunk)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(Box::pin(stream))
    }
}

/// Parse Anthropic SSE data
fn parse_anthropic_sse(bytes: &[u8]) -> Result<Option<StreamChunk>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| AiError::ProviderError(format!("Invalid UTF-8: {e}")))?;

    // SSE format: "event: event_type\ndata: {...}\n\n"
    let mut data_line = None;

    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            data_line = Some(data.to_string());
        }
    }

    if let Some(data) = data_line {
        let event: AnthropicStreamEvent = serde_json::from_str(&data).map_err(|e| {
            AiError::ProviderError(format!("Failed to parse event: {e} - data: {data}"))
        })?;

        match event {
            AnthropicStreamEvent::ContentBlockDelta { delta, .. } => {
                let ContentDelta::TextDelta { text } = delta;
                Ok(Some(StreamChunk {
                    delta: text,
                    is_final: false,
                    stop_reason: None,
                    index: 0,
                }))
            }
            AnthropicStreamEvent::MessageDelta { delta, .. } => Ok(Some(StreamChunk {
                delta: String::new(),
                is_final: delta.stop_reason.is_some(),
                stop_reason: delta.stop_reason,
                index: 0,
            })),
            AnthropicStreamEvent::MessageStop => Ok(Some(StreamChunk {
                delta: String::new(),
                is_final: true,
                stop_reason: Some("end_turn".to_string()),
                index: 0,
            })),
            AnthropicStreamEvent::Error { error } => Err(AiError::ProviderError(format!(
                "Anthropic stream error: {}",
                error.message
            ))),
            _ => Ok(None), // Skip other events
        }
    } else {
        Ok(None)
    }
}
