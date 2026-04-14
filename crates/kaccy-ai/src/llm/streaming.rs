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

#[cfg(test)]
mod tests {
    use super::*;

    // --- StreamChunk construction ---

    #[test]
    fn test_stream_chunk_non_final() {
        let chunk = StreamChunk {
            delta: "Hello".to_string(),
            is_final: false,
            stop_reason: None,
            index: 0,
        };
        assert_eq!(chunk.delta, "Hello");
        assert!(!chunk.is_final);
        assert!(chunk.stop_reason.is_none());
        assert_eq!(chunk.index, 0);
    }

    #[test]
    fn test_stream_chunk_final_with_stop_reason() {
        let chunk = StreamChunk {
            delta: " world".to_string(),
            is_final: true,
            stop_reason: Some("stop".to_string()),
            index: 0,
        };
        assert!(chunk.is_final);
        assert_eq!(chunk.stop_reason.as_deref(), Some("stop"));
    }

    // --- StreamAccumulator ---

    #[test]
    fn test_stream_accumulator_new_is_empty() {
        let acc = StreamAccumulator::new();
        assert!(acc.is_empty());
        assert_eq!(acc.len(), 0);
        assert_eq!(acc.text(), "");
    }

    #[test]
    fn test_stream_accumulator_add_single_chunk() {
        let mut acc = StreamAccumulator::new();
        let chunk = StreamChunk {
            delta: "Hello".to_string(),
            is_final: false,
            stop_reason: None,
            index: 0,
        };
        acc.add_chunk(&chunk);
        assert_eq!(acc.text(), "Hello");
        assert!(!acc.is_empty());
        assert_eq!(acc.len(), 5);
    }

    #[test]
    fn test_stream_accumulator_accumulates_multiple_deltas() {
        let mut acc = StreamAccumulator::new();
        for word in &["Hello", " ", "world", "!"] {
            acc.add_chunk(&StreamChunk {
                delta: word.to_string(),
                is_final: false,
                stop_reason: None,
                index: 0,
            });
        }
        assert_eq!(acc.text(), "Hello world!");
    }

    #[test]
    fn test_stream_accumulator_final_chunk_sets_stop_reason() {
        let mut acc = StreamAccumulator::new();
        acc.add_chunk(&StreamChunk {
            delta: "Done".to_string(),
            is_final: true,
            stop_reason: Some("end_turn".to_string()),
            index: 0,
        });
        let resp = acc.build();
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_stream_accumulator_non_final_chunk_does_not_set_stop_reason() {
        let mut acc = StreamAccumulator::new();
        acc.add_chunk(&StreamChunk {
            delta: "partial".to_string(),
            is_final: false,
            stop_reason: Some("should be ignored".to_string()),
            index: 0,
        });
        let resp = acc.build();
        // stop_reason only captured from is_final=true chunks
        assert!(resp.stop_reason.is_none());
    }

    #[test]
    fn test_stream_accumulator_set_usage() {
        let mut acc = StreamAccumulator::new();
        acc.set_usage(100, 50);
        let resp = acc.build();
        assert_eq!(resp.prompt_tokens, Some(100));
        assert_eq!(resp.completion_tokens, Some(50));
    }

    #[test]
    fn test_stream_accumulator_build_empty_stream() {
        let acc = StreamAccumulator::new();
        let resp = acc.build();
        assert_eq!(resp.text, "");
        assert!(resp.prompt_tokens.is_none());
        assert!(resp.completion_tokens.is_none());
        assert!(resp.stop_reason.is_none());
    }

    // --- From<StreamingChatResponse> for ChatMessage ---

    #[test]
    fn test_streaming_chat_response_into_chat_message_role_is_assistant() {
        let response = StreamingChatResponse {
            text: "I am the assistant.".to_string(),
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            stop_reason: Some("stop".to_string()),
        };
        let msg: ChatMessage = response.into();
        assert_eq!(msg.role, ChatRole::Assistant);
        assert_eq!(msg.content, "I am the assistant.");
    }

    // --- StreamingChatRequest ---

    #[test]
    fn test_streaming_chat_request_defaults_include_usage_true() {
        let req = StreamingChatRequest::with_system("system", "hello");
        assert!(req.include_usage);
    }

    #[test]
    fn test_streaming_chat_request_include_usage_toggle() {
        let req = StreamingChatRequest::with_system("system", "hello").include_usage(false);
        assert!(!req.include_usage);
    }

    // --- StreamHandler ---

    #[test]
    fn test_stream_handler_callback_invoked_and_accumulates() {
        use std::sync::{Arc, Mutex};
        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = Arc::clone(&captured);

        let mut handler = StreamHandler::new(move |chunk| {
            captured_clone
                .lock()
                .expect("lock captured")
                .push(chunk.delta.clone());
        });

        let chunks = vec![
            StreamChunk {
                delta: "A".to_string(),
                is_final: false,
                stop_reason: None,
                index: 0,
            },
            StreamChunk {
                delta: "B".to_string(),
                is_final: false,
                stop_reason: None,
                index: 0,
            },
            StreamChunk {
                delta: "C".to_string(),
                is_final: true,
                stop_reason: Some("stop".to_string()),
                index: 0,
            },
        ];

        for chunk in &chunks {
            handler.handle_chunk(chunk);
        }

        let resp = handler.finish();
        assert_eq!(resp.text, "ABC");
        assert_eq!(resp.stop_reason.as_deref(), Some("stop"));

        let received = captured.lock().expect("lock captured final");
        assert_eq!(*received, vec!["A", "B", "C"]);
    }
}
