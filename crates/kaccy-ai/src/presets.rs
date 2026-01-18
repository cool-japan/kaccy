//! Configuration presets for common use cases
//!
//! This module provides pre-configured setups for common AI service scenarios,
//! making it easier to get started with the right settings for your use case.

use std::collections::HashMap;
use std::time::Duration;

use crate::access_control::{AccessTier, AiFeature, FeatureQuota, TierConfig};
use crate::ai_evaluator::EvaluatorConfig;
use crate::batch::BatchConfig;
use crate::llm::{CircuitBreakerConfig, LlmCacheConfig, RateLimiterConfig, RetryConfig};
use crate::oracle::{ConsensusStrategy, OracleConfig};

/// Preset configuration for development/testing environments
pub struct DevelopmentPreset;

impl DevelopmentPreset {
    /// Get evaluator config for development (more verbose feedback)
    #[must_use]
    pub fn evaluator_config() -> EvaluatorConfig {
        EvaluatorConfig {
            max_response_tokens: 1000,
            temperature: 0.3,
            detailed_feedback: true,
        }
    }

    /// Get batch config for development (lower concurrency)
    #[must_use]
    pub fn batch_config() -> BatchConfig {
        BatchConfig {
            max_concurrent: 3,
            delay_between_items: Duration::from_millis(500),
            continue_on_error: true,
            max_retries: 1,
        }
    }

    /// Get retry config for development (fewer retries)
    #[must_use]
    pub fn retry_config() -> RetryConfig {
        RetryConfig {
            max_attempts: 2,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }

    /// Get rate limiter config for development (relaxed)
    #[must_use]
    pub fn rate_limiter_config() -> RateLimiterConfig {
        RateLimiterConfig::new(5.0).with_burst_size(10)
    }

    /// Get circuit breaker config for development
    #[must_use]
    pub fn circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
        }
    }

    /// Get cache config for development (shorter TTL)
    #[must_use]
    pub fn cache_config() -> LlmCacheConfig {
        LlmCacheConfig {
            max_entries: 100,
            ttl: Duration::from_secs(300), // 5 minutes
            semantic_matching: false,
            similarity_threshold: 0.9,
            max_prompt_tokens: 2000,
        }
    }
}

/// Preset configuration for production environments
pub struct ProductionPreset;

impl ProductionPreset {
    /// Get evaluator config for production (balanced)
    #[must_use]
    pub fn evaluator_config() -> EvaluatorConfig {
        EvaluatorConfig {
            max_response_tokens: 2000,
            temperature: 0.2,
            detailed_feedback: true,
        }
    }

    /// Get batch config for production (optimized for throughput)
    #[must_use]
    pub fn batch_config() -> BatchConfig {
        BatchConfig {
            max_concurrent: 10,
            delay_between_items: Duration::from_millis(100),
            continue_on_error: true,
            max_retries: 3,
        }
    }

    /// Get retry config for production (more resilient)
    #[must_use]
    pub fn retry_config() -> RetryConfig {
        RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }

    /// Get rate limiter config for production
    #[must_use]
    pub fn rate_limiter_config() -> RateLimiterConfig {
        RateLimiterConfig::new(10.0).with_burst_size(20)
    }

    /// Get circuit breaker config for production
    #[must_use]
    pub fn circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(120),
        }
    }

    /// Get cache config for production (longer TTL, more entries)
    #[must_use]
    pub fn cache_config() -> LlmCacheConfig {
        LlmCacheConfig {
            max_entries: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
            semantic_matching: true,
            similarity_threshold: 0.85,
            max_prompt_tokens: 4000,
        }
    }

    /// Get oracle config for production (multi-model consensus)
    #[must_use]
    pub fn oracle_config() -> OracleConfig {
        OracleConfig {
            strategy: ConsensusStrategy::Majority,
            model_weights: Vec::new(),
            min_confidence: 80.0,
            max_rejection_confidence: 30.0,
            enable_learning: true,
        }
    }
}

/// Preset configuration for high-volume environments
pub struct HighVolumePreset;

impl HighVolumePreset {
    /// Get evaluator config for high volume (concise feedback)
    #[must_use]
    pub fn evaluator_config() -> EvaluatorConfig {
        EvaluatorConfig {
            max_response_tokens: 1500,
            temperature: 0.2,
            detailed_feedback: false, // Faster, less detailed
        }
    }

    /// Get batch config for high volume (maximum throughput)
    #[must_use]
    pub fn batch_config() -> BatchConfig {
        BatchConfig {
            max_concurrent: 20,
            delay_between_items: Duration::from_millis(50),
            continue_on_error: true,
            max_retries: 2,
        }
    }

    /// Get retry config for high volume (balanced)
    #[must_use]
    pub fn retry_config() -> RetryConfig {
        RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.5,
            use_jitter: true,
        }
    }

    /// Get rate limiter config for high volume
    #[must_use]
    pub fn rate_limiter_config() -> RateLimiterConfig {
        RateLimiterConfig::new(50.0).with_burst_size(100)
    }

    /// Get circuit breaker config for high volume
    #[must_use]
    pub fn circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 20,
            success_threshold: 10,
            timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(90),
        }
    }

    /// Get cache config for high volume (aggressive caching)
    #[must_use]
    pub fn cache_config() -> LlmCacheConfig {
        LlmCacheConfig {
            max_entries: 5000,
            ttl: Duration::from_secs(7200), // 2 hours
            semantic_matching: true,
            similarity_threshold: 0.88,
            max_prompt_tokens: 3000,
        }
    }

    /// Get oracle config for high volume (weighted consensus for speed)
    #[must_use]
    pub fn oracle_config() -> OracleConfig {
        OracleConfig {
            strategy: ConsensusStrategy::WeightedAverage,
            model_weights: Vec::new(),
            min_confidence: 75.0,
            max_rejection_confidence: 35.0,
            enable_learning: true,
        }
    }
}

/// Preset configuration for cost-optimized environments
pub struct CostOptimizedPreset;

impl CostOptimizedPreset {
    /// Get evaluator config for cost optimization (minimal tokens)
    #[must_use]
    pub fn evaluator_config() -> EvaluatorConfig {
        EvaluatorConfig {
            max_response_tokens: 500,
            temperature: 0.1,
            detailed_feedback: false,
        }
    }

    /// Get batch config for cost optimization (lower concurrency)
    #[must_use]
    pub fn batch_config() -> BatchConfig {
        BatchConfig {
            max_concurrent: 3,
            delay_between_items: Duration::from_millis(300),
            continue_on_error: true,
            max_retries: 2,
        }
    }

    /// Get retry config for cost optimization
    #[must_use]
    pub fn retry_config() -> RetryConfig {
        RetryConfig {
            max_attempts: 2,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            use_jitter: false,
        }
    }

    /// Get rate limiter config for cost optimization
    #[must_use]
    pub fn rate_limiter_config() -> RateLimiterConfig {
        RateLimiterConfig::new(2.0).with_burst_size(5)
    }

    /// Get circuit breaker config for cost optimization
    #[must_use]
    pub fn circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(180),
        }
    }

    /// Get cache config for cost optimization (maximum caching)
    #[must_use]
    pub fn cache_config() -> LlmCacheConfig {
        LlmCacheConfig {
            max_entries: 10000,
            ttl: Duration::from_secs(86400), // 24 hours
            semantic_matching: true,
            similarity_threshold: 0.82, // More aggressive matching
            max_prompt_tokens: 5000,
        }
    }

    /// Get oracle config for cost optimization
    #[must_use]
    pub fn oracle_config() -> OracleConfig {
        OracleConfig {
            strategy: ConsensusStrategy::Majority,
            model_weights: Vec::new(),
            min_confidence: 90.0, // Higher threshold to reduce LLM calls
            max_rejection_confidence: 25.0,
            enable_learning: true,
        }
    }
}

/// Access control tier presets
pub struct AccessTierPresets;

impl AccessTierPresets {
    /// Get free tier configuration
    #[must_use]
    pub fn free_tier() -> TierConfig {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(10, 100));
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(5, 50),
        );

        TierConfig {
            tier: AccessTier::Free,
            feature_quotas: quotas,
            allow_custom_agents: false,
            max_custom_agents: 0,
        }
    }

    /// Get bronze tier configuration
    #[must_use]
    pub fn bronze_tier() -> TierConfig {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(50, 1000));
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(25, 500),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(20, 400));

        TierConfig {
            tier: AccessTier::Bronze,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 1,
        }
    }

    /// Get silver tier configuration
    #[must_use]
    pub fn silver_tier() -> TierConfig {
        let mut quotas = HashMap::new();
        quotas.insert(AiFeature::CodeEvaluation, FeatureQuota::limited(200, 5000));
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(100, 2500),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(100, 2500));
        quotas.insert(
            AiFeature::ReputationPrediction,
            FeatureQuota::limited(50, 1000),
        );

        TierConfig {
            tier: AccessTier::Silver,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 3,
        }
    }

    /// Get gold tier configuration
    #[must_use]
    pub fn gold_tier() -> TierConfig {
        let mut quotas = HashMap::new();
        quotas.insert(
            AiFeature::CodeEvaluation,
            FeatureQuota::limited(1000, 25000),
        );
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(500, 12500),
        );
        quotas.insert(AiFeature::FraudDetection, FeatureQuota::limited(500, 12500));
        quotas.insert(
            AiFeature::ReputationPrediction,
            FeatureQuota::limited(250, 6000),
        );
        quotas.insert(
            AiFeature::SentimentAnalysis,
            FeatureQuota::limited(200, 5000),
        );

        TierConfig {
            tier: AccessTier::Gold,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 10,
        }
    }

    /// Get platinum tier configuration (high limits)
    #[must_use]
    pub fn platinum_tier() -> TierConfig {
        let mut quotas = HashMap::new();
        quotas.insert(
            AiFeature::CodeEvaluation,
            FeatureQuota::limited(10000, 250_000),
        );
        quotas.insert(
            AiFeature::CommitmentVerification,
            FeatureQuota::limited(5000, 125_000),
        );
        quotas.insert(
            AiFeature::FraudDetection,
            FeatureQuota::limited(5000, 125_000),
        );
        quotas.insert(
            AiFeature::ReputationPrediction,
            FeatureQuota::limited(2500, 60000),
        );
        quotas.insert(
            AiFeature::SentimentAnalysis,
            FeatureQuota::limited(2000, 50000),
        );
        quotas.insert(AiFeature::CustomAgent, FeatureQuota::limited(1000, 25000));

        TierConfig {
            tier: AccessTier::Platinum,
            feature_quotas: quotas,
            allow_custom_agents: true,
            max_custom_agents: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_development_preset() {
        let config = DevelopmentPreset::evaluator_config();
        assert_eq!(config.max_response_tokens, 1000);
        assert_eq!(config.temperature, 0.3);
        assert!(config.detailed_feedback);

        let batch = DevelopmentPreset::batch_config();
        assert_eq!(batch.max_concurrent, 3);

        let retry = DevelopmentPreset::retry_config();
        assert_eq!(retry.max_attempts, 2);
    }

    #[test]
    fn test_production_preset() {
        let config = ProductionPreset::evaluator_config();
        assert_eq!(config.max_response_tokens, 2000);

        let batch = ProductionPreset::batch_config();
        assert_eq!(batch.max_concurrent, 10);

        let retry = ProductionPreset::retry_config();
        assert_eq!(retry.max_attempts, 5);
    }

    #[test]
    fn test_high_volume_preset() {
        let config = HighVolumePreset::evaluator_config();
        assert_eq!(config.max_response_tokens, 1500);
        assert!(!config.detailed_feedback); // Less detailed for speed

        let batch = HighVolumePreset::batch_config();
        assert_eq!(batch.max_concurrent, 20);
    }

    #[test]
    fn test_cost_optimized_preset() {
        let config = CostOptimizedPreset::evaluator_config();
        assert_eq!(config.max_response_tokens, 500); // Minimal tokens

        let batch = CostOptimizedPreset::batch_config();
        assert_eq!(batch.max_concurrent, 3);

        let cache = CostOptimizedPreset::cache_config();
        assert_eq!(cache.ttl, Duration::from_secs(86400)); // Long TTL
    }

    #[test]
    fn test_access_tier_presets() {
        let free = AccessTierPresets::free_tier();
        assert_eq!(free.tier, AccessTier::Free);
        assert_eq!(free.max_custom_agents, 0);
        assert!(!free.allow_custom_agents);

        let bronze = AccessTierPresets::bronze_tier();
        assert_eq!(bronze.tier, AccessTier::Bronze);
        assert_eq!(bronze.max_custom_agents, 1);
        assert!(bronze.allow_custom_agents);

        let platinum = AccessTierPresets::platinum_tier();
        assert_eq!(platinum.tier, AccessTier::Platinum);
        assert_eq!(platinum.max_custom_agents, 50);
        assert!(platinum.allow_custom_agents);
    }
}
