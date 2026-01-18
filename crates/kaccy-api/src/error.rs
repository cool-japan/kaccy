//! API error types

use std::collections::HashMap;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

/// A single field validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    /// The field name
    pub field: String,
    /// Error message for this field
    pub message: String,
    /// Error code (optional, for programmatic handling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl FieldError {
    /// Create a new field error
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: None,
        }
    }

    /// Create a new field error with a code
    pub fn with_code(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: Some(code.into()),
        }
    }
}

/// Detailed validation errors with field-level information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationErrors {
    /// Overall error message
    pub message: String,
    /// Per-field errors
    pub fields: Vec<FieldError>,
}

impl ValidationErrors {
    /// Create a new validation error with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fields: Vec::new(),
        }
    }

    /// Add a field error
    pub fn add_field(&mut self, field: impl Into<String>, message: impl Into<String>) -> &mut Self {
        self.fields.push(FieldError::new(field, message));
        self
    }

    /// Add a field error with code
    pub fn add_field_with_code(
        &mut self,
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
    ) -> &mut Self {
        self.fields
            .push(FieldError::with_code(field, message, code));
        self
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Convert to an API error if there are errors
    pub fn into_error(self) -> Option<ApiError> {
        if self.has_errors() {
            Some(ApiError::ValidationDetails(self))
        } else {
            None
        }
    }

    /// Get errors as a map of field -> messages
    pub fn as_map(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for err in &self.fields {
            map.entry(err.field.clone())
                .or_default()
                .push(err.message.clone());
        }
        map
    }
}

/// Builder for validation errors
pub struct ValidationBuilder {
    errors: ValidationErrors,
}

impl ValidationBuilder {
    /// Create a new validation builder
    pub fn new() -> Self {
        Self {
            errors: ValidationErrors::default(),
        }
    }

    /// Set the overall message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.errors.message = message.into();
        self
    }

    /// Add a field error
    pub fn field(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.errors.add_field(field, message);
        self
    }

    /// Add a required field error
    pub fn required(self, field: impl Into<String>) -> Self {
        let field = field.into();
        self.field(&field, format!("{} is required", field))
    }

    /// Add a min length error
    pub fn min_length(self, field: impl Into<String>, min: usize) -> Self {
        let field = field.into();
        self.field(
            &field,
            format!("{} must be at least {} characters", field, min),
        )
    }

    /// Add a max length error
    pub fn max_length(self, field: impl Into<String>, max: usize) -> Self {
        let field = field.into();
        self.field(
            &field,
            format!("{} must be at most {} characters", field, max),
        )
    }

    /// Add an invalid format error
    pub fn invalid_format(self, field: impl Into<String>, expected: impl Into<String>) -> Self {
        let field = field.into();
        self.field(
            &field,
            format!("{} has invalid format: {}", field, expected.into()),
        )
    }

    /// Build and return the validation errors
    pub fn build(self) -> ValidationErrors {
        self.errors
    }

    /// Build and return as an API error if there are any errors
    pub fn into_result<T>(self) -> Result<T> {
        if self.errors.has_errors() {
            Err(ApiError::ValidationDetails(self.errors))
        } else {
            // This should not be called without errors, but return a generic error
            Err(ApiError::Validation("Validation failed".to_string()))
        }
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.errors.has_errors()
    }
}

impl Default for ValidationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Database error: {0}")]
    DbError(#[from] kaccy_db::error::DbError),

    #[error("Core error: {0}")]
    Core(#[from] kaccy_core::CoreError),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Validation errors")]
    ValidationDetails(ValidationErrors),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal server error")]
    Internal,

    #[error("Internal server error: {0}")]
    InternalWithMessage(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
                let body = Json(json!({
                    "error": {
                        "message": "Database error",
                        "code": "DATABASE_ERROR"
                    }
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
            Self::DbError(ref e) => {
                tracing::error!("Database error: {:?}", e);
                let body = Json(json!({
                    "error": {
                        "message": "Database error",
                        "code": "DATABASE_ERROR"
                    }
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
            Self::Core(ref e) => {
                tracing::error!("Core error: {:?}", e);
                let body = Json(json!({
                    "error": {
                        "message": "Internal error",
                        "code": "INTERNAL_ERROR"
                    }
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
            Self::Auth(ref msg) => {
                let body = Json(json!({
                    "error": {
                        "message": msg,
                        "code": "AUTH_ERROR"
                    }
                }));
                (StatusCode::UNAUTHORIZED, body).into_response()
            }
            Self::Validation(ref msg) => {
                let body = Json(json!({
                    "error": {
                        "message": msg,
                        "code": "VALIDATION_ERROR"
                    }
                }));
                (StatusCode::BAD_REQUEST, body).into_response()
            }
            Self::ValidationDetails(ref errors) => {
                let body = Json(json!({
                    "error": {
                        "message": if errors.message.is_empty() { "Validation failed" } else { &errors.message },
                        "code": "VALIDATION_ERROR",
                        "details": errors.fields
                    }
                }));
                (StatusCode::BAD_REQUEST, body).into_response()
            }
            Self::NotFound(ref msg) => {
                let body = Json(json!({
                    "error": {
                        "message": msg,
                        "code": "NOT_FOUND"
                    }
                }));
                (StatusCode::NOT_FOUND, body).into_response()
            }
            Self::Forbidden(ref msg) => {
                let body = Json(json!({
                    "error": {
                        "message": msg,
                        "code": "FORBIDDEN"
                    }
                }));
                (StatusCode::FORBIDDEN, body).into_response()
            }
            Self::Internal => {
                tracing::error!("Internal server error");
                let body = Json(json!({
                    "error": {
                        "message": "Internal server error",
                        "code": "INTERNAL_ERROR"
                    }
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
            Self::InternalWithMessage(ref msg) => {
                tracing::error!("Internal server error: {}", msg);
                let body = Json(json!({
                    "error": {
                        "message": "Internal server error",
                        "code": "INTERNAL_ERROR"
                    }
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
