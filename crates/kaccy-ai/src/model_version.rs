//! Model version tracking and management
//!
//! This module provides version tracking for AI models, including performance metrics,
//! deployment history, and version comparison capabilities.
//!
//! # Examples
//!
//! ```
//! use kaccy_ai::model_version::{ModelRegistry, ModelVersion, ModelMetrics};
//!
//! let mut registry = ModelRegistry::new();
//!
//! // Register a new model version
//! let version = ModelVersion::new("gpt-4-turbo", "20240301", "GPT-4 Turbo March 2024");
//! registry.register_version(version).unwrap();
//!
//! // Record metrics
//! let metrics = ModelMetrics::new(95.5, 1250, 0.03);
//! registry.update_metrics("gpt-4-turbo", "20240301", metrics).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{AiError, Result};

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// Accuracy percentage (0-100)
    pub accuracy: f64,
    /// Average tokens per request
    pub avg_tokens: u32,
    /// Average cost per request (USD)
    pub avg_cost: f64,
    /// Total requests processed
    pub total_requests: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
}

impl ModelMetrics {
    /// Create new metrics
    #[must_use]
    pub fn new(accuracy: f64, avg_tokens: u32, avg_cost: f64) -> Self {
        Self {
            accuracy: accuracy.clamp(0.0, 100.0),
            avg_tokens,
            avg_cost,
            total_requests: 0,
            total_errors: 0,
            avg_latency_ms: 0.0,
        }
    }

    /// Create empty metrics
    #[must_use]
    pub fn empty() -> Self {
        Self {
            accuracy: 0.0,
            avg_tokens: 0,
            avg_cost: 0.0,
            total_requests: 0,
            total_errors: 0,
            avg_latency_ms: 0.0,
        }
    }

    /// Update with new request data
    pub fn record_request(&mut self, tokens: u32, cost: f64, latency_ms: f64, success: bool) {
        let n = self.total_requests as f64;

        // Update running averages
        self.avg_tokens = ((f64::from(self.avg_tokens) * n + f64::from(tokens)) / (n + 1.0)) as u32;
        self.avg_cost = (self.avg_cost * n + cost) / (n + 1.0);
        self.avg_latency_ms = (self.avg_latency_ms * n + latency_ms) / (n + 1.0);

        self.total_requests += 1;
        if !success {
            self.total_errors += 1;
        }

        // Update accuracy
        let success_count = self.total_requests - self.total_errors;
        self.accuracy = (success_count as f64 / self.total_requests as f64) * 100.0;
    }

    /// Get error rate percentage
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_errors as f64 / self.total_requests as f64) * 100.0
        }
    }

    /// Calculate cost-effectiveness score (accuracy per dollar)
    #[must_use]
    pub fn cost_effectiveness(&self) -> f64 {
        if self.avg_cost == 0.0 {
            0.0
        } else {
            self.accuracy / self.avg_cost
        }
    }
}

/// Model version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    /// Model family name (e.g., "gpt-4-turbo", "claude-3-opus")
    pub model_name: String,
    /// Version identifier (e.g., "20240301", "v1.2.3")
    pub version: String,
    /// Human-readable description
    pub description: String,
    /// Performance metrics
    pub metrics: ModelMetrics,
    /// Release date
    pub release_date: chrono::DateTime<chrono::Utc>,
    /// Deprecation date (if any)
    pub deprecated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether this version is currently active
    pub active: bool,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ModelVersion {
    /// Create a new model version
    pub fn new(
        model_name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            version: version.into(),
            description: description.into(),
            metrics: ModelMetrics::empty(),
            release_date: chrono::Utc::now(),
            deprecated_at: None,
            active: true,
            tags: Vec::new(),
        }
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set release date
    #[must_use]
    pub fn with_release_date(mut self, date: chrono::DateTime<chrono::Utc>) -> Self {
        self.release_date = date;
        self
    }

    /// Set initial metrics
    #[must_use]
    pub fn with_metrics(mut self, metrics: ModelMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Mark as deprecated
    pub fn deprecate(&mut self) {
        self.deprecated_at = Some(chrono::Utc::now());
        self.active = false;
    }

    /// Check if deprecated
    #[must_use]
    pub fn is_deprecated(&self) -> bool {
        self.deprecated_at.is_some()
    }

    /// Get unique identifier
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}:{}", self.model_name, self.version)
    }
}

/// Model registry for tracking versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    versions: HashMap<String, ModelVersion>,
    active_versions: HashMap<String, String>, // model_name -> active_version
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRegistry {
    /// Create a new model registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            versions: HashMap::new(),
            active_versions: HashMap::new(),
        }
    }

    /// Register a new model version
    pub fn register_version(&mut self, version: ModelVersion) -> Result<()> {
        let id = version.id();
        let model_name = version.model_name.clone();

        // Set as active version if it's the first or explicitly active
        if version.active {
            self.active_versions
                .insert(model_name, version.version.clone());
        }

        self.versions.insert(id, version);
        Ok(())
    }

    /// Get a specific version
    #[must_use]
    pub fn get_version(&self, model_name: &str, version: &str) -> Option<&ModelVersion> {
        let id = format!("{model_name}:{version}");
        self.versions.get(&id)
    }

    /// Get mutable version
    pub fn get_version_mut(
        &mut self,
        model_name: &str,
        version: &str,
    ) -> Option<&mut ModelVersion> {
        let id = format!("{model_name}:{version}");
        self.versions.get_mut(&id)
    }

    /// Get active version for a model
    #[must_use]
    pub fn get_active_version(&self, model_name: &str) -> Option<&ModelVersion> {
        let version = self.active_versions.get(model_name)?;
        self.get_version(model_name, version)
    }

    /// Set active version for a model
    pub fn set_active_version(&mut self, model_name: &str, version: &str) -> Result<()> {
        let id = format!("{model_name}:{version}");

        if !self.versions.contains_key(&id) {
            return Err(AiError::NotFound(format!("Model version {id} not found")));
        }

        self.active_versions
            .insert(model_name.to_string(), version.to_string());
        Ok(())
    }

    /// Get all versions for a model
    #[must_use]
    pub fn get_model_versions(&self, model_name: &str) -> Vec<&ModelVersion> {
        self.versions
            .values()
            .filter(|v| v.model_name == model_name)
            .collect()
    }

    /// Update metrics for a version
    pub fn update_metrics(
        &mut self,
        model_name: &str,
        version: &str,
        metrics: ModelMetrics,
    ) -> Result<()> {
        let id = format!("{model_name}:{version}");

        let model = self
            .versions
            .get_mut(&id)
            .ok_or_else(|| AiError::NotFound(format!("Model version {id} not found")))?;

        model.metrics = metrics;
        Ok(())
    }

    /// Record a request for a model version
    pub fn record_request(
        &mut self,
        model_name: &str,
        version: &str,
        tokens: u32,
        cost: f64,
        latency_ms: f64,
        success: bool,
    ) -> Result<()> {
        let id = format!("{model_name}:{version}");

        let model = self
            .versions
            .get_mut(&id)
            .ok_or_else(|| AiError::NotFound(format!("Model version {id} not found")))?;

        model
            .metrics
            .record_request(tokens, cost, latency_ms, success);
        Ok(())
    }

    /// Compare two model versions
    #[must_use]
    pub fn compare_versions(
        &self,
        model1: &str,
        version1: &str,
        model2: &str,
        version2: &str,
    ) -> Option<VersionComparison> {
        let v1 = self.get_version(model1, version1)?;
        let v2 = self.get_version(model2, version2)?;

        Some(VersionComparison {
            version1: v1.clone(),
            version2: v2.clone(),
            accuracy_diff: v1.metrics.accuracy - v2.metrics.accuracy,
            cost_diff: v1.metrics.avg_cost - v2.metrics.avg_cost,
            latency_diff: v1.metrics.avg_latency_ms - v2.metrics.avg_latency_ms,
        })
    }

    /// Get best performing version for a model (by accuracy)
    #[must_use]
    pub fn get_best_version(&self, model_name: &str) -> Option<&ModelVersion> {
        self.get_model_versions(model_name)
            .into_iter()
            .max_by(|a, b| a.metrics.accuracy.partial_cmp(&b.metrics.accuracy).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get most cost-effective version for a model
    #[must_use]
    pub fn get_most_cost_effective(&self, model_name: &str) -> Option<&ModelVersion> {
        self.get_model_versions(model_name)
            .into_iter()
            .max_by(|a, b| {
                a.metrics
                    .cost_effectiveness()
                    .partial_cmp(&b.metrics.cost_effectiveness())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get all model names
    #[must_use]
    pub fn list_models(&self) -> Vec<String> {
        let mut models: Vec<String> = self
            .versions
            .values()
            .map(|v| v.model_name.clone())
            .collect();
        models.sort();
        models.dedup();
        models
    }

    /// Deprecate a version
    pub fn deprecate_version(&mut self, model_name: &str, version: &str) -> Result<()> {
        let id = format!("{model_name}:{version}");

        let model = self
            .versions
            .get_mut(&id)
            .ok_or_else(|| AiError::NotFound(format!("Model version {id} not found")))?;

        model.deprecate();

        // If this was the active version, clear it
        if let Some(active) = self.active_versions.get(model_name) {
            if active == version {
                self.active_versions.remove(model_name);
            }
        }

        Ok(())
    }

    /// Save registry to file
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AiError::Internal(format!("Failed to serialize registry: {e}")))?;

        std::fs::write(path, json)
            .map_err(|e| AiError::Internal(format!("Failed to write registry: {e}")))?;

        Ok(())
    }

    /// Load registry from file
    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| AiError::Internal(format!("Failed to read registry: {e}")))?;

        let registry: ModelRegistry = serde_json::from_str(&json)
            .map_err(|e| AiError::Internal(format!("Failed to deserialize registry: {e}")))?;

        Ok(registry)
    }

    /// Get total number of versions
    #[must_use]
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }
}

/// Comparison between two model versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionComparison {
    /// First version being compared.
    pub version1: ModelVersion,
    /// Second version being compared.
    pub version2: ModelVersion,
    /// Accuracy difference (version1 - version2)
    pub accuracy_diff: f64,
    /// Cost difference (version1 - version2)
    pub cost_diff: f64,
    /// Latency difference (version1 - version2)
    pub latency_diff: f64,
}

impl VersionComparison {
    /// Get recommendation based on comparison
    #[must_use]
    pub fn recommendation(&self) -> &'static str {
        if self.accuracy_diff > 5.0 && self.cost_diff < 0.01 {
            "Version 1 is significantly more accurate with similar cost"
        } else if self.accuracy_diff < -5.0 && self.cost_diff > -0.01 {
            "Version 2 is significantly more accurate with similar cost"
        } else if self.cost_diff < -0.005 && self.accuracy_diff.abs() < 2.0 {
            "Version 1 is more cost-effective with similar accuracy"
        } else if self.cost_diff > 0.005 && self.accuracy_diff.abs() < 2.0 {
            "Version 2 is more cost-effective with similar accuracy"
        } else {
            "Versions have similar performance characteristics"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metrics_creation() {
        let metrics = ModelMetrics::new(95.5, 1000, 0.02);
        assert_eq!(metrics.accuracy, 95.5);
        assert_eq!(metrics.avg_tokens, 1000);
        assert_eq!(metrics.avg_cost, 0.02);
    }

    #[test]
    fn test_model_metrics_record_request() {
        let mut metrics = ModelMetrics::empty();

        metrics.record_request(1000, 0.02, 150.0, true);
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.accuracy, 100.0);

        metrics.record_request(1200, 0.025, 180.0, false);
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.total_errors, 1);
        assert_eq!(metrics.accuracy, 50.0);
    }

    #[test]
    fn test_model_version_creation() {
        let version = ModelVersion::new("gpt-4-turbo", "20240301", "GPT-4 Turbo March");
        assert_eq!(version.model_name, "gpt-4-turbo");
        assert_eq!(version.version, "20240301");
        assert!(version.active);
        assert!(!version.is_deprecated());
    }

    #[test]
    fn test_model_registry() {
        let mut registry = ModelRegistry::new();

        let version = ModelVersion::new("gpt-4-turbo", "20240301", "Test version");
        registry.register_version(version).unwrap();

        assert_eq!(registry.len(), 1);

        let retrieved = registry.get_version("gpt-4-turbo", "20240301");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_active_version() {
        let mut registry = ModelRegistry::new();

        let v1 = ModelVersion::new("gpt-4", "v1", "Version 1");
        let v2 = ModelVersion::new("gpt-4", "v2", "Version 2");

        registry.register_version(v1).unwrap();
        registry.register_version(v2).unwrap();

        registry.set_active_version("gpt-4", "v2").unwrap();

        let active = registry.get_active_version("gpt-4").unwrap();
        assert_eq!(active.version, "v2");
    }

    #[test]
    fn test_version_comparison() {
        let mut registry = ModelRegistry::new();

        let v1 = ModelVersion::new("gpt-4", "v1", "V1")
            .with_metrics(ModelMetrics::new(90.0, 1000, 0.02));
        let v2 = ModelVersion::new("gpt-4", "v2", "V2")
            .with_metrics(ModelMetrics::new(95.0, 1200, 0.025));

        registry.register_version(v1).unwrap();
        registry.register_version(v2).unwrap();

        let comparison = registry
            .compare_versions("gpt-4", "v1", "gpt-4", "v2")
            .unwrap();
        assert!(comparison.accuracy_diff < 0.0); // v2 is more accurate
    }

    #[test]
    fn test_deprecation() {
        let mut registry = ModelRegistry::new();

        let version = ModelVersion::new("gpt-3.5", "old", "Old version");
        registry.register_version(version).unwrap();

        registry.deprecate_version("gpt-3.5", "old").unwrap();

        let deprecated = registry.get_version("gpt-3.5", "old").unwrap();
        assert!(deprecated.is_deprecated());
        assert!(!deprecated.active);
    }

    #[test]
    fn test_best_version() {
        let mut registry = ModelRegistry::new();

        registry
            .register_version(
                ModelVersion::new("claude", "v1", "V1")
                    .with_metrics(ModelMetrics::new(85.0, 1000, 0.02)),
            )
            .unwrap();

        registry
            .register_version(
                ModelVersion::new("claude", "v2", "V2")
                    .with_metrics(ModelMetrics::new(95.0, 1200, 0.03)),
            )
            .unwrap();

        let best = registry.get_best_version("claude").unwrap();
        assert_eq!(best.version, "v2");
    }

    #[test]
    fn test_registry_persistence() {
        let mut registry = ModelRegistry::new();

        registry
            .register_version(ModelVersion::new("test-model", "v1.0", "Test"))
            .unwrap();

        let temp_path = "/tmp/model_registry_test.json";
        registry.save_to_file(temp_path).unwrap();

        let loaded = ModelRegistry::load_from_file(temp_path).unwrap();
        assert_eq!(loaded.len(), 1);

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }
}
