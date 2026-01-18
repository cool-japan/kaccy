//! Serialization helpers for API responses and DTOs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Response data (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_metadata(data: T, metadata: HashMap<String, serde_json::Value>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
            metadata: Some(metadata),
        }
    }

    /// Create an error response
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create an error response with metadata
    pub fn error_with_metadata(
        error: String,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: Utc::now(),
            metadata: Some(metadata),
        }
    }

    /// Add metadata to the response
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add a single metadata entry
    pub fn with_meta(mut self, key: String, value: serde_json::Value) -> Self {
        let mut metadata = self.metadata.unwrap_or_default();
        metadata.insert(key, value);
        self.metadata = Some(metadata);
        self
    }
}

/// Paginated API response wrapper (alias to avoid naming conflict)
pub use crate::utils::pagination::PaginatedResponse as ApiPaginatedResponse;

/// Trait for converting models to DTOs
pub trait ToDto<T> {
    /// Convert to DTO
    fn to_dto(&self) -> T;
}

/// Trait for converting DTOs to models
pub trait FromDto<T> {
    /// Convert from DTO
    fn from_dto(dto: T) -> Self;
}

/// Helper for partial updates using JSON Merge Patch (RFC 7396)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartialUpdate<T> {
    fields: HashMap<String, serde_json::Value>,
    #[serde(skip)]
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PartialUpdate<T> {
    /// Create a new partial update
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set a field value
    pub fn set<V: Serialize>(mut self, key: impl Into<String>, value: V) -> Self {
        let json_value = serde_json::to_value(value).expect("Failed to serialize value");
        self.fields.insert(key.into(), json_value);
        self
    }

    /// Remove a field (set to null)
    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.fields.insert(key.into(), serde_json::Value::Null);
        self
    }

    /// Get all fields
    pub fn fields(&self) -> &HashMap<String, serde_json::Value> {
        &self.fields
    }

    /// Check if a field is set
    pub fn has_field(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Get a field value
    pub fn get_field(&self, key: &str) -> Option<&serde_json::Value> {
        self.fields.get(key)
    }

    /// Apply the partial update to a JSON value
    pub fn apply_to(&self, mut target: serde_json::Value) -> serde_json::Value {
        if let serde_json::Value::Object(ref mut map) = target {
            for (key, value) in &self.fields {
                if value.is_null() {
                    map.remove(key);
                } else {
                    map.insert(key.clone(), value.clone());
                }
            }
        }
        target
    }
}

impl<T> Default for PartialUpdate<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// Successfully processed items
    pub succeeded: Vec<T>,
    /// Failed items with error messages
    pub failed: Vec<BatchError>,
    /// Total count
    pub total: usize,
    /// Success count
    pub success_count: usize,
    /// Failure count
    pub failure_count: usize,
}

/// Batch error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchError {
    /// Index of the failed item
    pub index: usize,
    /// Error message
    pub error: String,
}

impl<T> BatchResult<T> {
    /// Create a new batch result
    pub fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
            total: 0,
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Add a successful item
    pub fn add_success(&mut self, item: T) {
        self.succeeded.push(item);
        self.success_count += 1;
        self.total += 1;
    }

    /// Add a failed item
    pub fn add_failure(&mut self, index: usize, error: String) {
        self.failed.push(BatchError { index, error });
        self.failure_count += 1;
        self.total += 1;
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failure_count == 0
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success_count as f64 / self.total as f64) * 100.0
        }
    }
}

impl<T> Default for BatchResult<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_api_response_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), json!("value"));

        let response = ApiResponse::success("test").with_metadata(metadata.clone());

        assert!(response.metadata.is_some());
        assert_eq!(response.metadata.unwrap().get("key"), metadata.get("key"));
    }

    #[test]
    fn test_partial_update() {
        let update = PartialUpdate::<String>::new()
            .set("name", "John")
            .set("age", 30);

        assert!(update.has_field("name"));
        assert!(update.has_field("age"));
        assert!(!update.has_field("email"));
    }

    #[test]
    fn test_partial_update_apply() {
        let update = PartialUpdate::<String>::new()
            .set("name", "John")
            .remove("old_field");

        let original = json!({
            "name": "Jane",
            "age": 25,
            "old_field": "remove me"
        });

        let updated = update.apply_to(original);
        assert_eq!(updated["name"], "John");
        assert_eq!(updated["age"], 25);
        assert!(updated.get("old_field").is_none());
    }

    #[test]
    fn test_batch_result() {
        let mut result = BatchResult::<i32>::new();
        result.add_success(1);
        result.add_success(2);
        result.add_failure(2, "Error processing item".to_string());

        assert_eq!(result.total, 3);
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 1);
        assert!(!result.all_succeeded());
        assert!((result.success_rate() - 66.67).abs() < 0.1);
    }
}
