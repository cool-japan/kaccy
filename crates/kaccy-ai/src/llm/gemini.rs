//! Google Gemini API client
//!
//! Integration with Google's Gemini models with streaming support.

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

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini API client
#[derive(Clone)]
pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    /// Create a new Gemini client
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Create with default model (gemini-1.5-pro)
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "gemini-1.5-pro")
    }

    /// Create with Gemini 1.5 Flash (faster, cheaper)
    pub fn with_flash(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "gemini-1.5-flash")
    }

    /// Create with Gemini 2.0 Flash (latest)
    pub fn with_2_0_flash(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "gemini-2.0-flash-exp")
    }

    /// Set a different model
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl LlmProvider for GeminiClient {
    fn name(&self) -> &'static str {
        "gemini"
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
        // Convert messages to Gemini format
        let contents: Vec<GeminiContent> = request
            .messages
            .iter()
            .filter(|m| m.role != ChatRole::System) // Gemini doesn't have system role
            .map(|m| GeminiContent {
                role: match m.role {
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "model".to_string(),
                    ChatRole::System => "user".to_string(), // Fallback
                },
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            })
            .collect();

        // Handle system messages by prepending them to the first user message
        let system_msg: Option<String> = request
            .messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .map(|m| m.content.clone());

        let mut contents = contents;
        if let Some(system) = system_msg {
            if let Some(first) = contents.first_mut() {
                if first.role == "user" {
                    first.parts[0].text = format!("{}\n\n{}", system, first.parts[0].text);
                }
            }
        }

        let mut generation_config = GeminiGenerationConfig::default();
        if let Some(temp) = request.temperature {
            generation_config.temperature = Some(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            generation_config.max_output_tokens = Some(max_tokens as usize);
        }
        if let Some(stop) = request.stop {
            generation_config.stop_sequences = Some(stop);
        }

        let api_request = GeminiGenerateContentRequest {
            contents,
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            GEMINI_API_URL, self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Gemini request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Gemini API error ({status}): {error_text}"
            )));
        }

        let api_response: GeminiGenerateContentResponse = response
            .json()
            .await
            .map_err(|e| AiError::ProviderError(format!("Failed to parse Gemini response: {e}")))?;

        let candidate = api_response.candidates.into_iter().next().ok_or_else(|| {
            AiError::ProviderError("No candidates in Gemini response".to_string())
        })?;

        let content_text = candidate
            .content
            .parts
            .into_iter()
            .map(|p| p.text)
            .collect::<String>();

        // Calculate token counts (Gemini provides usage metadata)
        let prompt_tokens = api_response
            .usage_metadata
            .as_ref()
            .map_or(0, |u| u.prompt_token_count);
        let completion_tokens = api_response
            .usage_metadata
            .as_ref()
            .map_or(0, |u| u.candidates_token_count);

        Ok(ChatResponse {
            message: ChatMessage {
                role: ChatRole::Assistant,
                content: content_text,
            },
            prompt_tokens: prompt_tokens as u32,
            completion_tokens: completion_tokens as u32,
            total_tokens: (prompt_tokens + completion_tokens) as u32,
            finish_reason: Some(
                candidate
                    .finish_reason
                    .unwrap_or_else(|| "stop".to_string()),
            ),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        // Check if we can list models
        let url = format!("{}/models?key={}", GEMINI_API_URL, self.api_key);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Gemini health check failed: {e}")))?;

        Ok(response.status().is_success())
    }

    fn clone_box(&self) -> Box<dyn LlmProvider> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl StreamingLlmProvider for GeminiClient {
    async fn chat_stream(&self, request: StreamingChatRequest) -> Result<StreamResponse> {
        // Extract the base chat request
        let chat_request = request.request;

        // Convert messages to Gemini format
        let contents: Vec<GeminiContent> = chat_request
            .messages
            .iter()
            .filter(|m| m.role != ChatRole::System)
            .map(|m| GeminiContent {
                role: match m.role {
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "model".to_string(),
                    ChatRole::System => "user".to_string(),
                },
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            })
            .collect();

        let mut generation_config = GeminiGenerationConfig::default();
        if let Some(temp) = chat_request.temperature {
            generation_config.temperature = Some(temp);
        }
        if let Some(max_tokens) = chat_request.max_tokens {
            generation_config.max_output_tokens = Some(max_tokens as usize);
        }

        let api_request = GeminiGenerateContentRequest {
            contents,
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            GEMINI_API_URL, self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&api_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError(format!("Gemini streaming request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AiError::ProviderError(format!(
                "Gemini streaming API error ({status}): {error_text}"
            )));
        }

        let stream = response.bytes_stream().map(move |result| match result {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                // Parse SSE format
                if let Some(data) = text.strip_prefix("data: ") {
                    if let Ok(chunk_response) =
                        serde_json::from_str::<GeminiGenerateContentResponse>(data)
                    {
                        if let Some(candidate) = chunk_response.candidates.first() {
                            let chunk_text = candidate
                                .content
                                .parts
                                .iter()
                                .map(|p| p.text.as_str())
                                .collect::<Vec<_>>()
                                .join("");

                            let is_final = candidate.finish_reason.is_some();
                            return Ok(StreamChunk {
                                delta: chunk_text,
                                is_final,
                                stop_reason: candidate.finish_reason.clone(),
                                index: 0,
                            });
                        }
                    }
                }
                Ok(StreamChunk {
                    delta: String::new(),
                    is_final: false,
                    stop_reason: None,
                    index: 0,
                })
            }
            Err(e) => Err(AiError::ProviderError(format!("Stream error: {e}"))),
        });

        Ok(Box::pin(stream))
    }
}

// API types

#[derive(Debug, Serialize)]
struct GeminiGenerateContentRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Default)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateContentResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: usize,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GeminiClient::new("test-key", "gemini-1.5-pro");
        assert_eq!(client.name(), "gemini");
        assert_eq!(client.model, "gemini-1.5-pro");
    }

    #[test]
    fn test_default_model() {
        let client = GeminiClient::with_default_model("test-key");
        assert_eq!(client.model, "gemini-1.5-pro");
    }

    #[test]
    fn test_flash_model() {
        let client = GeminiClient::with_flash("test-key");
        assert_eq!(client.model, "gemini-1.5-flash");
    }

    #[test]
    fn test_2_0_flash_model() {
        let client = GeminiClient::with_2_0_flash("test-key");
        assert_eq!(client.model, "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_model_setter() {
        let client = GeminiClient::with_default_model("test-key").model("custom-model");
        assert_eq!(client.model, "custom-model");
    }

    #[test]
    fn test_clone() {
        let client = GeminiClient::new("test-key", "gemini-1.5-pro");
        let cloned = client.clone();
        assert_eq!(cloned.model, client.model);
        assert_eq!(cloned.name(), client.name());
    }

    #[test]
    fn test_gemini_content_serialization() {
        let content = GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart {
                text: "Hello".to_string(),
            }],
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_generation_config_default() {
        let config = GeminiGenerationConfig::default();
        assert!(config.temperature.is_none());
        assert!(config.max_output_tokens.is_none());
        assert!(config.stop_sequences.is_none());
    }
}
