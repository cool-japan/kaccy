//! Streaming support for LLM providers
//!
//! Provides real-time token-by-token streaming for chat completions.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use super::types::{ChatMessage, ChatRequest, ChatRole};
use crate::error::Result;

/// A chunk of streamed response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// The text delta for this chunk
    pub delta: String,
    /// Whether this is the final chunk
    pub is_final: bool,
    /// Stop reason (only set on final chunk)
    pub stop_reason: Option<String>,
    /// Index of the choice (for multi-response)
    pub index: u32,
}

/// Final streaming response with usage info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingChatResponse {
    /// Complete accumulated text
    pub text: String,
    /// Prompt tokens used
    pub prompt_tokens: Option<u32>,
    /// Completion tokens generated
    pub completion_tokens: Option<u32>,
    /// Stop reason
    pub stop_reason: Option<String>,
}

/// Streaming request configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingChatRequest {
    /// Base chat request
    pub request: ChatRequest,
    /// Include usage information in final message
    pub include_usage: bool,
}

impl StreamingChatRequest {
    /// Create a new streaming request from a chat request
    #[must_use]
    pub fn new(request: ChatRequest) -> Self {
        Self {
            request,
            include_usage: true,
        }
    }

    /// Create with system prompt and user message
    pub fn with_system(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self::new(ChatRequest::with_system(system, user))
    }

    /// Set whether to include usage info
    #[must_use]
    pub fn include_usage(mut self, include: bool) -> Self {
        self.include_usage = include;
        self
    }
}

/// Type alias for streaming response
pub type StreamResponse = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>;

/// Trait for LLM providers that support streaming
#[async_trait]
pub trait StreamingLlmProvider: Send + Sync {
    /// Stream a chat completion response
    async fn chat_stream(&self, request: StreamingChatRequest) -> Result<StreamResponse>;
}

/// Accumulator for building complete response from stream
pub struct StreamAccumulator {
    text: String,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    stop_reason: Option<String>,
}

impl Default for StreamAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAccumulator {
    /// Create a new accumulator
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            prompt_tokens: None,
            completion_tokens: None,
            stop_reason: None,
        }
    }

    /// Add a chunk to the accumulator
    pub fn add_chunk(&mut self, chunk: &StreamChunk) {
        self.text.push_str(&chunk.delta);
        if chunk.is_final {
            self.stop_reason = chunk.stop_reason.clone();
        }
    }

    /// Set usage information
    pub fn set_usage(&mut self, prompt_tokens: u32, completion_tokens: u32) {
        self.prompt_tokens = Some(prompt_tokens);
        self.completion_tokens = Some(completion_tokens);
    }

    /// Build the final response
    #[must_use]
    pub fn build(self) -> StreamingChatResponse {
        StreamingChatResponse {
            text: self.text,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            stop_reason: self.stop_reason,
        }
    }

    /// Get the accumulated text so far
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get current length
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Helper to convert a streaming response to a complete message
pub async fn collect_stream(mut stream: StreamResponse) -> Result<StreamingChatResponse> {
    use futures::StreamExt;

    let mut accumulator = StreamAccumulator::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        accumulator.add_chunk(&chunk);
    }

    Ok(accumulator.build())
}

/// Convert streaming response to `ChatMessage`
impl From<StreamingChatResponse> for ChatMessage {
    fn from(response: StreamingChatResponse) -> Self {
        ChatMessage {
            role: ChatRole::Assistant,
            content: response.text,
        }
    }
}

/// Callback type for handling stream chunks
pub type StreamCallback = Box<dyn Fn(&StreamChunk) + Send + Sync>;

/// Stream handler that calls a callback for each chunk
pub struct StreamHandler {
    callback: StreamCallback,
    accumulator: StreamAccumulator,
}

impl StreamHandler {
    /// Create a new stream handler with a callback
    pub fn new(callback: impl Fn(&StreamChunk) + Send + Sync + 'static) -> Self {
        Self {
            callback: Box::new(callback),
            accumulator: StreamAccumulator::new(),
        }
    }

    /// Process a chunk
    pub fn handle_chunk(&mut self, chunk: &StreamChunk) {
        (self.callback)(chunk);
        self.accumulator.add_chunk(chunk);
    }

    /// Get the accumulated response
    #[must_use]
    pub fn finish(self) -> StreamingChatResponse {
        self.accumulator.build()
    }
}

/// Simple print handler for debugging
#[must_use]
pub fn print_handler() -> StreamHandler {
    StreamHandler::new(|chunk| {
        print!("{}", chunk.delta);
        if chunk.is_final {
            println!();
        }
    })
}
