//! Common types for LLM interactions

use serde::{Deserialize, Serialize};

/// Role in a chat conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// System / instruction message.
    System,
    /// Human turn.
    User,
    /// Model turn.
    Assistant,
}

/// A message in a chat conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message author.
    pub role: ChatRole,
    /// Text content of the message.
    pub content: String,
}

impl ChatMessage {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// Request for text completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The prompt to complete
    pub prompt: String,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0-2.0, lower = more deterministic)
    pub temperature: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
}

impl CompletionRequest {
    /// Create a new completion request with default parameters.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            stop: None,
        }
    }

    /// Override the maximum number of tokens to generate.
    #[must_use]
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Override the sampling temperature.
    #[must_use]
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set stop sequences that will halt generation.
    #[must_use]
    pub fn stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop = Some(sequences);
        self
    }
}

/// Response from text completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Generated text
    pub text: String,
    /// Tokens used in the prompt
    pub prompt_tokens: u32,
    /// Tokens generated
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
    /// Stop reason
    pub finish_reason: Option<String>,
}

/// Image content in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// Image URL (can be data URI with base64)
    pub url: String,
    /// Optional detail level for image analysis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

/// Level of detail for image analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// Low detail - faster and cheaper
    Low,
    /// High detail - more accurate
    High,
    /// Auto - let the model decide
    Auto,
}

/// Request for chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Conversation messages
    pub messages: Vec<ChatMessage>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0-2.0, lower = more deterministic)
    pub temperature: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Images to include in the request (for vision models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageContent>>,
}

impl ChatRequest {
    /// Create a new chat request with default parameters.
    #[must_use]
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            stop: None,
            images: None,
        }
    }

    /// Create a two-message request with a system prompt and a user message.
    pub fn with_system(system_prompt: impl Into<String>, user_message: impl Into<String>) -> Self {
        Self::new(vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(user_message),
        ])
    }

    /// Create a vision request with an image
    pub fn with_vision(
        system_prompt: impl Into<String>,
        user_message: impl Into<String>,
        image_url: impl Into<String>,
    ) -> Self {
        Self {
            messages: vec![
                ChatMessage::system(system_prompt),
                ChatMessage::user(user_message),
            ],
            max_tokens: Some(4096),
            temperature: Some(0.3),
            stop: None,
            images: Some(vec![ImageContent {
                url: image_url.into(),
                detail: Some(ImageDetail::Auto),
            }]),
        }
    }

    /// Add an image to the request
    #[must_use]
    pub fn with_image(mut self, url: impl Into<String>) -> Self {
        let image = ImageContent {
            url: url.into(),
            detail: Some(ImageDetail::Auto),
        };
        if let Some(ref mut images) = self.images {
            images.push(image);
        } else {
            self.images = Some(vec![image]);
        }
        self
    }

    /// Add an image with specific detail level
    #[must_use]
    pub fn with_image_detail(mut self, url: impl Into<String>, detail: ImageDetail) -> Self {
        let image = ImageContent {
            url: url.into(),
            detail: Some(detail),
        };
        if let Some(ref mut images) = self.images {
            images.push(image);
        } else {
            self.images = Some(vec![image]);
        }
        self
    }

    /// Override the maximum number of tokens to generate.
    #[must_use]
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Override the sampling temperature.
    #[must_use]
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Append a message to the conversation.
    #[must_use]
    pub fn add_message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Check if this is a vision request
    #[must_use]
    pub fn is_vision_request(&self) -> bool {
        self.images.as_ref().is_some_and(|i| !i.is_empty())
    }
}

/// Response from chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Assistant's response message
    pub message: ChatMessage,
    /// Tokens used in the prompt
    pub prompt_tokens: u32,
    /// Tokens generated
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
    /// Stop reason
    pub finish_reason: Option<String>,
}

/// Model capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier
    pub id: String,
    /// Maximum context length
    pub context_length: u32,
    /// Cost per 1000 input tokens (USD)
    pub input_cost_per_1k: f64,
    /// Cost per 1000 output tokens (USD)
    pub output_cost_per_1k: f64,
    /// Provider name
    pub provider: String,
}

impl ModelInfo {
    /// Preset for `gpt-4-turbo`.
    #[must_use]
    pub fn gpt4_turbo() -> Self {
        Self {
            id: "gpt-4-turbo".to_string(),
            context_length: 128_000,
            input_cost_per_1k: 0.01,
            output_cost_per_1k: 0.03,
            provider: "openai".to_string(),
        }
    }

    /// Preset for `gpt-4o`.
    #[must_use]
    pub fn gpt4o() -> Self {
        Self {
            id: "gpt-4o".to_string(),
            context_length: 128_000,
            input_cost_per_1k: 0.005,
            output_cost_per_1k: 0.015,
            provider: "openai".to_string(),
        }
    }

    /// Preset for `claude-3-opus`.
    #[must_use]
    pub fn claude_3_opus() -> Self {
        Self {
            id: "claude-3-opus-20240229".to_string(),
            context_length: 200_000,
            input_cost_per_1k: 0.015,
            output_cost_per_1k: 0.075,
            provider: "anthropic".to_string(),
        }
    }

    /// Preset for `claude-3-sonnet`.
    #[must_use]
    pub fn claude_3_sonnet() -> Self {
        Self {
            id: "claude-3-sonnet-20240229".to_string(),
            context_length: 200_000,
            input_cost_per_1k: 0.003,
            output_cost_per_1k: 0.015,
            provider: "anthropic".to_string(),
        }
    }

    /// Preset for `claude-3-5-sonnet`.
    #[must_use]
    pub fn claude_3_5_sonnet() -> Self {
        Self {
            id: "claude-3-5-sonnet-20241022".to_string(),
            context_length: 200_000,
            input_cost_per_1k: 0.003,
            output_cost_per_1k: 0.015,
            provider: "anthropic".to_string(),
        }
    }

    /// Preset for `gemini-1.5-pro`.
    #[must_use]
    pub fn gemini_1_5_pro() -> Self {
        Self {
            id: "gemini-1.5-pro".to_string(),
            context_length: 1_048_576, // 1M context window
            input_cost_per_1k: 0.00125,
            output_cost_per_1k: 0.005,
            provider: "gemini".to_string(),
        }
    }

    /// Preset for `gemini-1.5-flash`.
    #[must_use]
    pub fn gemini_1_5_flash() -> Self {
        Self {
            id: "gemini-1.5-flash".to_string(),
            context_length: 1_048_576, // 1M context window
            input_cost_per_1k: 0.000_075,
            output_cost_per_1k: 0.0003,
            provider: "gemini".to_string(),
        }
    }

    /// Preset for `gemini-2.0-flash` (experimental).
    #[must_use]
    pub fn gemini_2_0_flash() -> Self {
        Self {
            id: "gemini-2.0-flash-exp".to_string(),
            context_length: 1_048_576, // 1M context window
            input_cost_per_1k: 0.0,    // Experimental/free tier
            output_cost_per_1k: 0.0,
            provider: "gemini".to_string(),
        }
    }

    /// Preset for `deepseek-chat`.
    #[must_use]
    pub fn deepseek_chat() -> Self {
        Self {
            id: "deepseek-chat".to_string(),
            context_length: 32_768,
            input_cost_per_1k: 0.00014,
            output_cost_per_1k: 0.00028,
            provider: "deepseek".to_string(),
        }
    }

    /// Preset for `deepseek-coder`.
    #[must_use]
    pub fn deepseek_coder() -> Self {
        Self {
            id: "deepseek-coder".to_string(),
            context_length: 32_768,
            input_cost_per_1k: 0.00014,
            output_cost_per_1k: 0.00028,
            provider: "deepseek".to_string(),
        }
    }

    /// Preset for `deepseek-reasoner`.
    #[must_use]
    pub fn deepseek_reasoner() -> Self {
        Self {
            id: "deepseek-reasoner".to_string(),
            context_length: 64_000,
            input_cost_per_1k: 0.00055,
            output_cost_per_1k: 0.00219,
            provider: "deepseek".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ChatMessage construction ---

    #[test]
    fn test_chat_message_system_role_and_content() {
        let msg = ChatMessage::system("You are a helpful assistant.");
        assert_eq!(msg.role, ChatRole::System);
        assert_eq!(msg.content, "You are a helpful assistant.");
    }

    #[test]
    fn test_chat_message_user_role_and_content() {
        let msg = ChatMessage::user("Hello!");
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.content, "Hello!");
    }

    #[test]
    fn test_chat_message_assistant_role_and_content() {
        let msg = ChatMessage::assistant("Hi there!");
        assert_eq!(msg.role, ChatRole::Assistant);
        assert_eq!(msg.content, "Hi there!");
    }

    // --- ChatRole serde (lowercase rename) ---

    #[test]
    fn test_chat_role_serde_lowercase() {
        let serialized =
            serde_json::to_string(&ChatRole::System).expect("serialize ChatRole::System");
        assert_eq!(serialized, "\"system\"");

        let serialized = serde_json::to_string(&ChatRole::User).expect("serialize ChatRole::User");
        assert_eq!(serialized, "\"user\"");

        let serialized =
            serde_json::to_string(&ChatRole::Assistant).expect("serialize ChatRole::Assistant");
        assert_eq!(serialized, "\"assistant\"");
    }

    #[test]
    fn test_chat_role_deserialize_lowercase() {
        let role: ChatRole = serde_json::from_str("\"system\"").expect("deserialize system");
        assert_eq!(role, ChatRole::System);

        let role: ChatRole = serde_json::from_str("\"user\"").expect("deserialize user");
        assert_eq!(role, ChatRole::User);

        let role: ChatRole = serde_json::from_str("\"assistant\"").expect("deserialize assistant");
        assert_eq!(role, ChatRole::Assistant);
    }

    // --- CompletionRequest builder ---

    #[test]
    fn test_completion_request_defaults() {
        let req = CompletionRequest::new("test prompt");
        assert_eq!(req.prompt, "test prompt");
        assert_eq!(req.max_tokens, Some(1024));
        assert!(req.temperature.is_some());
        assert!(req.stop.is_none());
    }

    #[test]
    fn test_completion_request_builder_chain() {
        let req = CompletionRequest::new("prompt")
            .max_tokens(512)
            .temperature(0.2)
            .stop_sequences(vec!["END".to_string(), "STOP".to_string()]);
        assert_eq!(req.max_tokens, Some(512));
        assert!((req.temperature.expect("temperature present") - 0.2_f32).abs() < 1e-6);
        let stop = req.stop.expect("stop sequences present");
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0], "END");
    }

    // --- ChatRequest construction ---

    #[test]
    fn test_chat_request_with_system() {
        let req = ChatRequest::with_system("sys", "user msg");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, ChatRole::System);
        assert_eq!(req.messages[1].role, ChatRole::User);
        assert!(!req.is_vision_request());
    }

    #[test]
    fn test_chat_request_defaults() {
        let req = ChatRequest::new(vec![ChatMessage::user("hello")]);
        assert_eq!(req.max_tokens, Some(1024));
        assert!(req.temperature.is_some());
        assert!(req.images.is_none());
    }

    #[test]
    fn test_chat_request_is_vision_request_false_when_no_images() {
        let req = ChatRequest::new(vec![ChatMessage::user("no images")]);
        assert!(!req.is_vision_request());
    }

    #[test]
    fn test_chat_request_with_vision() {
        let req = ChatRequest::with_vision("sys", "describe image", "https://example.com/img.png");
        assert!(req.is_vision_request());
        let images = req.images.expect("images present in vision request");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].url, "https://example.com/img.png");
    }

    #[test]
    fn test_chat_request_with_image_adds_to_existing() {
        let req = ChatRequest::with_vision("sys", "msg", "https://example.com/a.png")
            .with_image("https://example.com/b.png");
        let images = req.images.expect("images present");
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn test_chat_request_with_image_detail() {
        let req = ChatRequest::new(vec![ChatMessage::user("hi")])
            .with_image_detail("https://example.com/img.png", ImageDetail::High);
        assert!(req.is_vision_request());
        let images = req.images.expect("images present");
        assert!(matches!(images[0].detail, Some(ImageDetail::High)));
    }

    #[test]
    fn test_chat_request_add_message() {
        let req = ChatRequest::new(vec![ChatMessage::user("first")])
            .add_message(ChatMessage::assistant("response"));
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[1].role, ChatRole::Assistant);
    }

    // --- ChatMessage serde round-trip ---

    #[test]
    fn test_chat_message_serde_roundtrip() {
        let original = ChatMessage::user("round-trip content");
        let json = serde_json::to_string(&original).expect("serialize ChatMessage");
        let deserialized: ChatMessage =
            serde_json::from_str(&json).expect("deserialize ChatMessage");
        assert_eq!(deserialized.content, original.content);
        assert_eq!(deserialized.role, original.role);
    }

    // --- CompletionResponse field access ---

    #[test]
    fn test_completion_response_deserialization() {
        let json = r#"{
            "text": "Hello world",
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15,
            "finish_reason": "stop"
        }"#;
        let resp: CompletionResponse =
            serde_json::from_str(json).expect("deserialize CompletionResponse");
        assert_eq!(resp.text, "Hello world");
        assert_eq!(resp.prompt_tokens, 10);
        assert_eq!(resp.completion_tokens, 5);
        assert_eq!(resp.total_tokens, 15);
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn test_completion_response_no_finish_reason() {
        let json = r#"{
            "text": "partial",
            "prompt_tokens": 5,
            "completion_tokens": 3,
            "total_tokens": 8,
            "finish_reason": null
        }"#;
        let resp: CompletionResponse =
            serde_json::from_str(json).expect("deserialize CompletionResponse null finish");
        assert!(resp.finish_reason.is_none());
    }

    // --- ModelInfo factory spot-check ---

    #[test]
    fn test_model_info_gpt4_turbo() {
        let info = ModelInfo::gpt4_turbo();
        assert_eq!(info.id, "gpt-4-turbo");
        assert_eq!(info.provider, "openai");
        assert_eq!(info.context_length, 128_000);
        assert!((info.input_cost_per_1k - 0.01_f64).abs() < 1e-9);
        assert!((info.output_cost_per_1k - 0.03_f64).abs() < 1e-9);
    }

    #[test]
    fn test_model_info_claude_3_opus() {
        let info = ModelInfo::claude_3_opus();
        assert_eq!(info.provider, "anthropic");
        assert_eq!(info.context_length, 200_000);
    }

    #[test]
    fn test_model_info_gemini_1_5_pro_context_length() {
        let info = ModelInfo::gemini_1_5_pro();
        assert_eq!(info.context_length, 1_048_576);
        assert_eq!(info.provider, "gemini");
    }

    // --- ChatResponse deserialization ---

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "message": {"role": "assistant", "content": "I can help."},
            "prompt_tokens": 20,
            "completion_tokens": 8,
            "total_tokens": 28,
            "finish_reason": "stop"
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).expect("deserialize ChatResponse");
        assert_eq!(resp.message.role, ChatRole::Assistant);
        assert_eq!(resp.message.content, "I can help.");
        assert_eq!(resp.prompt_tokens, 20);
        assert_eq!(resp.completion_tokens, 8);
        assert_eq!(resp.total_tokens, 28);
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    }
}
