//! Cost optimization for LLM requests
//!
//! This module provides utilities to minimize LLM API costs through:
//! - Request batching: Aggregate multiple evaluations into single requests
//! - Model routing: Use cheaper models for simple tasks, escalate to powerful models when needed
//! - Smart caching: Leverage semantic similarity for cache hits (uses existing cache module)

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

use super::{ChatRequest, ChatResponse, LlmProvider};
use crate::error::{AiError, Result};

/// Complexity level for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskComplexity {
    /// Simple tasks (e.g., classification, basic extraction)
    Simple,
    /// Medium tasks (e.g., summarization, basic analysis)
    Medium,
    /// Complex tasks (e.g., deep reasoning, code generation)
    Complex,
}

/// Model tier for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTier {
    /// Model identifier
    pub model: String,
    /// Cost per 1K tokens (input)
    pub cost_per_1k_input: f64,
    /// Cost per 1K tokens (output)
    pub cost_per_1k_output: f64,
    /// Maximum complexity this model can handle
    pub max_complexity: TaskComplexity,
}

impl ModelTier {
    /// GPT-3.5 Turbo tier (cheap, simple tasks)
    #[must_use]
    pub fn gpt_3_5_turbo() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            cost_per_1k_input: 0.0005,
            cost_per_1k_output: 0.0015,
            max_complexity: TaskComplexity::Simple,
        }
    }

    /// GPT-4 Turbo tier (medium cost, complex tasks)
    #[must_use]
    pub fn gpt_4_turbo() -> Self {
        Self {
            model: "gpt-4-turbo".to_string(),
            cost_per_1k_input: 0.01,
            cost_per_1k_output: 0.03,
            max_complexity: TaskComplexity::Complex,
        }
    }

    /// Claude Haiku tier (very cheap, simple tasks)
    #[must_use]
    pub fn claude_haiku() -> Self {
        Self {
            model: "claude-3-haiku-20240307".to_string(),
            cost_per_1k_input: 0.00025,
            cost_per_1k_output: 0.00125,
            max_complexity: TaskComplexity::Simple,
        }
    }

    /// Claude Sonnet tier (medium cost, medium-complex tasks)
    #[must_use]
    pub fn claude_sonnet() -> Self {
        Self {
            model: "claude-3-5-sonnet-20241022".to_string(),
            cost_per_1k_input: 0.003,
            cost_per_1k_output: 0.015,
            max_complexity: TaskComplexity::Medium,
        }
    }

    /// Claude Opus tier (expensive, most complex tasks)
    #[must_use]
    pub fn claude_opus() -> Self {
        Self {
            model: "claude-3-opus-20240229".to_string(),
            cost_per_1k_input: 0.015,
            cost_per_1k_output: 0.075,
            max_complexity: TaskComplexity::Complex,
        }
    }

    /// Gemini 1.5 Flash tier (very cheap, simple-medium tasks)
    #[must_use]
    pub fn gemini_1_5_flash() -> Self {
        Self {
            model: "gemini-1.5-flash".to_string(),
            cost_per_1k_input: 0.000_075,
            cost_per_1k_output: 0.0003,
            max_complexity: TaskComplexity::Medium,
        }
    }

    /// Gemini 1.5 Pro tier (medium cost, complex tasks)
    #[must_use]
    pub fn gemini_1_5_pro() -> Self {
        Self {
            model: "gemini-1.5-pro".to_string(),
            cost_per_1k_input: 0.00125,
            cost_per_1k_output: 0.005,
            max_complexity: TaskComplexity::Complex,
        }
    }

    /// Gemini 2.0 Flash tier (experimental/free, simple-medium tasks)
    #[must_use]
    pub fn gemini_2_0_flash() -> Self {
        Self {
            model: "gemini-2.0-flash-exp".to_string(),
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            max_complexity: TaskComplexity::Medium,
        }
    }

    /// `DeepSeek` Chat tier (very cheap, simple-medium tasks)
    #[must_use]
    pub fn deepseek_chat() -> Self {
        Self {
            model: "deepseek-chat".to_string(),
            cost_per_1k_input: 0.00014,
            cost_per_1k_output: 0.00028,
            max_complexity: TaskComplexity::Medium,
        }
    }

    /// `DeepSeek` Coder tier (very cheap, optimized for code)
    #[must_use]
    pub fn deepseek_coder() -> Self {
        Self {
            model: "deepseek-coder".to_string(),
            cost_per_1k_input: 0.00014,
            cost_per_1k_output: 0.00028,
            max_complexity: TaskComplexity::Medium,
        }
    }

    /// `DeepSeek` Reasoner tier (cheap, complex reasoning tasks)
    #[must_use]
    pub fn deepseek_reasoner() -> Self {
        Self {
            model: "deepseek-reasoner".to_string(),
            cost_per_1k_input: 0.00055,
            cost_per_1k_output: 0.00219,
            max_complexity: TaskComplexity::Complex,
        }
    }
}

/// Configuration for model routing
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Available model tiers (ordered from cheapest to most expensive)
    pub tiers: Vec<ModelTier>,
    /// Auto-escalate if confidence is low
    pub auto_escalate: bool,
    /// Confidence threshold for escalation (0-100)
    pub escalation_threshold: f64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            tiers: vec![
                ModelTier::claude_haiku(),
                ModelTier::claude_sonnet(),
                ModelTier::claude_opus(),
            ],
            auto_escalate: true,
            escalation_threshold: 70.0,
        }
    }
}

impl RoutingConfig {
    /// Get the appropriate model for a given complexity
    #[must_use]
    pub fn model_for_complexity(&self, complexity: TaskComplexity) -> Option<&ModelTier> {
        self.tiers
            .iter()
            .find(|tier| tier.max_complexity as u8 >= complexity as u8)
    }

    /// Estimate cost for a request
    #[must_use]
    pub fn estimate_cost(&self, model: &str, input_tokens: usize, output_tokens: usize) -> f64 {
        if let Some(tier) = self.tiers.iter().find(|t| t.model == model) {
            let input_cost = (input_tokens as f64 / 1000.0) * tier.cost_per_1k_input;
            let output_cost = (output_tokens as f64 / 1000.0) * tier.cost_per_1k_output;
            input_cost + output_cost
        } else {
            0.0
        }
    }
}

/// Model router that selects appropriate models based on task complexity
pub struct ModelRouter {
    config: RoutingConfig,
    providers: Vec<Box<dyn LlmProvider>>,
}

impl ModelRouter {
    /// Create a new model router
    #[must_use]
    pub fn new(config: RoutingConfig, providers: Vec<Box<dyn LlmProvider>>) -> Self {
        Self { config, providers }
    }

    /// Select the best model for a task
    #[must_use]
    pub fn select_model(&self, complexity: TaskComplexity) -> Option<&str> {
        self.config
            .model_for_complexity(complexity)
            .map(|tier| tier.model.as_str())
    }

    /// Route a chat request to the appropriate model
    pub async fn route_chat(
        &self,
        request: ChatRequest,
        complexity: TaskComplexity,
    ) -> Result<ChatResponse> {
        let model = self
            .select_model(complexity)
            .ok_or_else(|| AiError::Configuration("No suitable model found".to_string()))?;

        // Model selection is handled by the provider based on their configuration
        // Try each provider until one succeeds
        for provider in &self.providers {
            match provider.chat(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!(
                        model = model,
                        provider = provider.name(),
                        error = %e,
                        "Provider failed, trying next"
                    );
                }
            }
        }

        Err(AiError::Unavailable(format!(
            "No provider available for model: {model}"
        )))
    }
}

/// Batched request item
#[derive(Debug, Clone)]
pub struct BatchItem<T> {
    /// Unique identifier for this item
    pub id: String,
    /// The request payload
    pub request: T,
    /// Task complexity (for routing)
    pub complexity: TaskComplexity,
}

/// Batch processor for aggregating multiple requests
#[allow(clippy::type_complexity)]
pub struct BatchProcessor<T, R> {
    /// Pending items
    pending: Arc<Mutex<Vec<BatchItem<T>>>>,
    /// Maximum batch size
    max_batch_size: usize,
    /// Maximum wait time before processing batch
    max_wait_ms: u64,
    /// Processing function
    processor: Arc<dyn Fn(Vec<BatchItem<T>>) -> Vec<(String, Result<R>)> + Send + Sync>,
}

impl<T, R> BatchProcessor<T, R>
where
    T: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    /// Create a new batch processor
    pub fn new<F>(max_batch_size: usize, max_wait_ms: u64, processor: F) -> Self
    where
        F: Fn(Vec<BatchItem<T>>) -> Vec<(String, Result<R>)> + Send + Sync + 'static,
    {
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
            max_batch_size,
            max_wait_ms,
            processor: Arc::new(processor),
        }
    }

    /// Add an item to the batch
    pub async fn add(&self, item: BatchItem<T>) -> Result<R> {
        let mut pending = self.pending.lock().await;
        let item_id = item.id.clone();
        pending.push(item);

        // Check if we should process immediately
        if pending.len() >= self.max_batch_size {
            let batch = std::mem::take(&mut *pending);
            drop(pending); // Release lock before processing

            return self.process_batch(batch, &item_id);
        }

        // Wait for batch window
        drop(pending);
        sleep(Duration::from_millis(self.max_wait_ms)).await;

        // Process batch
        let mut pending = self.pending.lock().await;
        let batch = std::mem::take(&mut *pending);
        drop(pending);

        self.process_batch(batch, &item_id)
    }

    fn process_batch(&self, batch: Vec<BatchItem<T>>, item_id: &str) -> Result<R> {
        if batch.is_empty() {
            return Err(AiError::InvalidInput("Empty batch".to_string()));
        }

        let results = (self.processor)(batch);

        // Find result for this item
        results
            .into_iter()
            .find(|(id, _)| id == item_id)
            .map(|(_, result)| result)
            .unwrap_or_else(|| {
                Err(AiError::Internal(
                    "Item not found in batch results".to_string(),
                ))
            })
    }
}

/// Cost tracking for monitoring API usage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostTracker {
    /// Total input tokens used
    pub total_input_tokens: usize,
    /// Total output tokens used
    pub total_output_tokens: usize,
    /// Total estimated cost (USD)
    pub total_cost: f64,
    /// Requests by model
    pub requests_by_model: std::collections::HashMap<String, usize>,
}

impl CostTracker {
    /// Create a new cost tracker
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a request
    pub fn record_request(
        &mut self,
        model: &str,
        input_tokens: usize,
        output_tokens: usize,
        cost: f64,
    ) {
        self.total_input_tokens += input_tokens;
        self.total_output_tokens += output_tokens;
        self.total_cost += cost;
        *self.requests_by_model.entry(model.to_string()).or_insert(0) += 1;
    }

    /// Get total tokens used
    #[must_use]
    pub fn total_tokens(&self) -> usize {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Get average cost per request
    #[must_use]
    pub fn avg_cost_per_request(&self) -> f64 {
        let total_requests: usize = self.requests_by_model.values().sum();
        if total_requests == 0 {
            0.0
        } else {
            self.total_cost / total_requests as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_tier_costs() {
        let haiku = ModelTier::claude_haiku();
        assert_eq!(haiku.max_complexity, TaskComplexity::Simple);
        assert!(haiku.cost_per_1k_input < 0.001);

        let opus = ModelTier::claude_opus();
        assert_eq!(opus.max_complexity, TaskComplexity::Complex);
        assert!(opus.cost_per_1k_input > haiku.cost_per_1k_input);
    }

    #[test]
    fn test_routing_config_model_selection() {
        let config = RoutingConfig::default();

        let simple_model = config.model_for_complexity(TaskComplexity::Simple);
        assert!(simple_model.is_some());
        assert_eq!(simple_model.unwrap().model, "claude-3-haiku-20240307");

        let complex_model = config.model_for_complexity(TaskComplexity::Complex);
        assert!(complex_model.is_some());
    }

    #[test]
    fn test_cost_estimation() {
        let config = RoutingConfig::default();
        let cost = config.estimate_cost("claude-3-haiku-20240307", 1000, 500);
        assert!(cost > 0.0);
        assert!(cost < 1.0); // Should be very cheap for small requests
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new();
        tracker.record_request("claude-3-haiku-20240307", 1000, 500, 0.5);
        tracker.record_request("claude-3-opus-20240229", 2000, 1000, 2.0);

        assert_eq!(tracker.total_input_tokens, 3000);
        assert_eq!(tracker.total_output_tokens, 1500);
        assert_eq!(tracker.total_cost, 2.5);
        assert_eq!(tracker.total_tokens(), 4500);
        assert_eq!(tracker.avg_cost_per_request(), 1.25);
    }

    #[test]
    fn test_complexity_ordering() {
        assert!((TaskComplexity::Simple as u8) < (TaskComplexity::Medium as u8));
        assert!((TaskComplexity::Medium as u8) < (TaskComplexity::Complex as u8));
    }
}
