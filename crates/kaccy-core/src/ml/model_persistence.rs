//! Model Persistence and Versioning
//!
//! Provides functionality to save, load, and version machine learning models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Model metadata for versioning and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Unique model ID
    pub model_id: Uuid,
    /// Model name
    pub name: String,
    /// Model version
    pub version: String,
    /// Model type
    pub model_type: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Model parameters
    pub parameters: HashMap<String, String>,
    /// Model performance metrics
    pub metrics: HashMap<String, f64>,
    /// Description
    pub description: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl ModelMetadata {
    /// Create a new model metadata record with version 1.0.0
    pub fn new(name: String, model_type: String) -> Self {
        let now = Utc::now();
        Self {
            model_id: Uuid::new_v4(),
            name,
            version: "1.0.0".to_string(),
            model_type,
            created_at: now,
            updated_at: now,
            parameters: HashMap::new(),
            metrics: HashMap::new(),
            description: None,
            tags: Vec::new(),
        }
    }

    /// Set the version string on this metadata
    pub fn with_version(mut self, version: String) -> Self {
        self.version = version;
        self
    }

    /// Add a named hyperparameter to this metadata
    pub fn with_parameter(mut self, key: String, value: String) -> Self {
        self.parameters.insert(key, value);
        self
    }

    /// Add a named performance metric to this metadata
    pub fn with_metric(mut self, key: String, value: f64) -> Self {
        self.metrics.insert(key, value);
        self
    }

    /// Set a human-readable description on this metadata
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the categorization tags on this metadata
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Update the version string and refresh the `updated_at` timestamp
    pub fn update_version(&mut self, new_version: String) {
        self.version = new_version;
        self.updated_at = Utc::now();
    }

    /// Replace all performance metrics and refresh the `updated_at` timestamp
    pub fn update_metrics(&mut self, metrics: HashMap<String, f64>) {
        self.metrics = metrics;
        self.updated_at = Utc::now();
    }
}

/// Trait for models that can be persisted
pub trait PersistentModel: Serialize + for<'de> Deserialize<'de> {
    /// Get model metadata
    fn metadata(&self) -> &ModelMetadata;

    /// Get mutable model metadata
    fn metadata_mut(&mut self) -> &mut ModelMetadata;

    /// Save model to file
    fn save(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load model from file
    fn load(path: &Path) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let json = std::fs::read_to_string(path)?;
        let model = serde_json::from_str(&json)?;
        Ok(model)
    }
}

/// Model version manager
#[derive(Debug)]
pub struct ModelVersionManager {
    /// Base directory for storing models
    base_dir: PathBuf,
    /// Active models indexed by name
    active_models: HashMap<String, ModelVersion>,
}

/// Model version information
#[derive(Debug, Clone)]
pub struct ModelVersion {
    /// Metadata describing this version
    pub metadata: ModelMetadata,
    /// Filesystem path to the serialized model file
    pub path: PathBuf,
    /// Whether this is the currently active version
    pub is_active: bool,
}

impl ModelVersionManager {
    /// Create a new model version manager rooted at the given directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            active_models: HashMap::new(),
        }
    }

    /// Register a new model version
    pub fn register_version(
        &mut self,
        name: String,
        metadata: ModelMetadata,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        let version = ModelVersion {
            metadata,
            path,
            is_active: true,
        };

        // Deactivate previous version if exists
        if let Some(prev) = self.active_models.get_mut(&name) {
            prev.is_active = false;
        }

        self.active_models.insert(name, version);
        Ok(())
    }

    /// Get active model version
    pub fn get_active_version(&self, name: &str) -> Option<&ModelVersion> {
        self.active_models.get(name).filter(|v| v.is_active)
    }

    /// List all registered models
    pub fn list_models(&self) -> Vec<&ModelVersion> {
        self.active_models.values().collect()
    }

    /// Get model path for saving
    pub fn get_model_path(&self, name: &str, version: &str) -> PathBuf {
        self.base_dir
            .join(name)
            .join(format!("model-v{}.json", version))
    }

    /// Create directory structure for model
    pub fn create_model_dir(&self, name: &str) -> anyhow::Result<()> {
        let dir = self.base_dir.join(name);
        std::fs::create_dir_all(&dir)?;
        Ok(())
    }
}

/// Model comparison result
#[derive(Debug, Clone)]
pub struct ModelComparison {
    /// Name of the first model
    pub model_a: String,
    /// Name of the second model
    pub model_b: String,
    /// Per-metric differences (model_a value minus model_b value)
    pub metric_diffs: HashMap<String, f64>,
    /// Name of the model that performed better on the comparison metric, if any
    pub better_model: Option<String>,
}

impl ModelComparison {
    /// Compare two models based on a specific metric
    pub fn compare(
        metadata_a: &ModelMetadata,
        metadata_b: &ModelMetadata,
        metric_name: &str,
    ) -> Self {
        let mut metric_diffs = HashMap::new();

        // Calculate differences for all common metrics
        for (key, value_a) in &metadata_a.metrics {
            if let Some(value_b) = metadata_b.metrics.get(key) {
                metric_diffs.insert(key.clone(), value_a - value_b);
            }
        }

        // Determine better model based on specified metric
        let better_model = if let Some(&diff) = metric_diffs.get(metric_name) {
            if diff > 0.0 {
                Some(metadata_a.name.clone())
            } else if diff < 0.0 {
                Some(metadata_b.name.clone())
            } else {
                None // Equal performance
            }
        } else {
            None
        };

        Self {
            model_a: metadata_a.name.clone(),
            model_b: metadata_b.name.clone(),
            metric_diffs,
            better_model,
        }
    }
}

/// Model registry for managing multiple models
#[derive(Debug)]
pub struct ModelRegistry {
    /// Registered models
    models: HashMap<Uuid, ModelMetadata>,
    /// Models indexed by name and version
    name_index: HashMap<String, Vec<Uuid>>,
}

impl ModelRegistry {
    /// Create a new empty model registry
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    /// Register a model
    pub fn register(&mut self, metadata: ModelMetadata) {
        let model_id = metadata.model_id;
        let name = metadata.name.clone();

        self.models.insert(model_id, metadata);
        self.name_index.entry(name).or_default().push(model_id);
    }

    /// Get model by ID
    pub fn get_by_id(&self, model_id: Uuid) -> Option<&ModelMetadata> {
        self.models.get(&model_id)
    }

    /// Get all versions of a model by name
    pub fn get_versions(&self, name: &str) -> Vec<&ModelMetadata> {
        self.name_index
            .get(name)
            .map(|ids| ids.iter().filter_map(|id| self.models.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get latest version by name
    pub fn get_latest(&self, name: &str) -> Option<&ModelMetadata> {
        let mut versions = self.get_versions(name);
        versions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        versions.first().copied()
    }

    /// Search models by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&ModelMetadata> {
        self.models
            .values()
            .filter(|m| m.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Get model count
    pub fn count(&self) -> usize {
        self.models.len()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metadata_creation() {
        let metadata = ModelMetadata::new("test_model".to_string(), "linear".to_string())
            .with_version("1.0.0".to_string())
            .with_parameter("learning_rate".to_string(), "0.01".to_string())
            .with_metric("accuracy".to_string(), 0.95);

        assert_eq!(metadata.name, "test_model");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(
            metadata.parameters.get("learning_rate"),
            Some(&"0.01".to_string())
        );
        assert_eq!(metadata.metrics.get("accuracy"), Some(&0.95));
    }

    #[test]
    fn test_model_version_update() {
        let mut metadata = ModelMetadata::new("test_model".to_string(), "linear".to_string());
        let original_updated = metadata.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        metadata.update_version("2.0.0".to_string());

        assert_eq!(metadata.version, "2.0.0");
        assert!(metadata.updated_at > original_updated);
    }

    #[test]
    fn test_model_version_manager() {
        let temp_dir = std::env::temp_dir().join("test_models");
        let mut manager = ModelVersionManager::new(&temp_dir);

        let metadata = ModelMetadata::new("test_model".to_string(), "linear".to_string());
        let path = temp_dir.join("test_model").join("model-v1.0.0.json");

        manager
            .register_version("test_model".to_string(), metadata.clone(), path.clone())
            .unwrap();

        let active = manager.get_active_version("test_model");
        assert!(active.is_some());
        assert_eq!(active.unwrap().metadata.name, "test_model");
    }

    #[test]
    fn test_model_comparison() {
        let mut metadata_a = ModelMetadata::new("model_a".to_string(), "linear".to_string());
        metadata_a.metrics.insert("accuracy".to_string(), 0.95);
        metadata_a.metrics.insert("precision".to_string(), 0.92);

        let mut metadata_b = ModelMetadata::new("model_b".to_string(), "linear".to_string());
        metadata_b.metrics.insert("accuracy".to_string(), 0.90);
        metadata_b.metrics.insert("precision".to_string(), 0.93);

        let comparison = ModelComparison::compare(&metadata_a, &metadata_b, "accuracy");

        assert_eq!(comparison.better_model, Some("model_a".to_string()));
        let accuracy_diff = comparison.metric_diffs.get("accuracy").unwrap();
        assert!((accuracy_diff - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_model_registry() {
        let mut registry = ModelRegistry::new();

        let metadata1 = ModelMetadata::new("test_model".to_string(), "linear".to_string())
            .with_version("1.0.0".to_string())
            .with_tags(vec!["production".to_string()]);

        let metadata2 = ModelMetadata::new("test_model".to_string(), "linear".to_string())
            .with_version("2.0.0".to_string())
            .with_tags(vec!["production".to_string()]);

        registry.register(metadata1.clone());
        registry.register(metadata2.clone());

        assert_eq!(registry.count(), 2);

        let versions = registry.get_versions("test_model");
        assert_eq!(versions.len(), 2);

        let prod_models = registry.search_by_tag("production");
        assert_eq!(prod_models.len(), 2);
    }
}
