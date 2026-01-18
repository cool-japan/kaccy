//! Common types for LLM interactions

use serde::{Deserialize, Serialize};

/// Role in a chat conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// A message in a chat conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

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
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            stop: None,
        }
    }

    #[must_use]
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    #[must_use]
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

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

    #[must_use]
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    #[must_use]
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

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
