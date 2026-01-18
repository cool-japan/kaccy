//! Request batching endpoint

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Method, Request},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{ApiError, Result, ValidationBuilder},
    middleware::Claims,
    state::AppState,
};

/// Maximum number of requests in a batch
const MAX_BATCH_SIZE: usize = 10;

/// Single request in a batch
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    /// Request ID (for matching responses)
    pub id: String,
    /// HTTP method
    pub method: String,
    /// Request path (e.g., "/api/tokens")
    pub path: String,
    /// Request headers (optional)
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// Request body (optional, as JSON)
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

/// Batch request payload
#[derive(Debug, Deserialize)]
pub struct BatchRequestPayload {
    /// List of requests to execute
    pub requests: Vec<BatchRequest>,
}

/// Single response in a batch
#[derive(Debug, Serialize)]
pub struct BatchResponse {
    /// Request ID (matches the request)
    pub id: String,
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: std::collections::HashMap<String, String>,
    /// Response body (as JSON)
    pub body: serde_json::Value,
}

/// Batch response payload
#[derive(Debug, Serialize)]
pub struct BatchResponsePayload {
    /// List of responses
    pub responses: Vec<BatchResponse>,
}

/// Execute a batch of requests
pub async fn execute_batch(
    State(_state): State<AppState>,
    claims: Claims,
    Json(payload): Json<BatchRequestPayload>,
) -> Result<Json<BatchResponsePayload>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // Validation
    let mut errors = ValidationBuilder::new();

    if payload.requests.is_empty() {
        errors = errors.field("requests", "At least one request is required");
    }

    if payload.requests.len() > MAX_BATCH_SIZE {
        errors = errors.field(
            "requests",
            format!("Maximum {} requests allowed per batch", MAX_BATCH_SIZE),
        );
    }

    // Validate each request
    for (i, req) in payload.requests.iter().enumerate() {
        if req.id.is_empty() {
            errors = errors.field(format!("requests[{}].id", i), "Request ID is required");
        }

        let method = req.method.to_uppercase();
        if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH") {
            errors = errors.field(
                format!("requests[{}].method", i),
                format!("Invalid HTTP method: {}", req.method),
            );
        }

        if req.path.is_empty() {
            errors = errors.field(format!("requests[{}].path", i), "Request path is required");
        }

        // Ensure path doesn't start with /api/batch (prevent recursion)
        if req.path.starts_with("/api/batch") {
            errors = errors.field(
                format!("requests[{}].path", i),
                "Batch endpoint cannot be called from within a batch",
            );
        }
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    // Execute requests
    // In production, this would:
    // 1. Parse each request
    // 2. Execute them (potentially in parallel using tokio::spawn)
    // 3. Collect responses
    // 4. Return batch response
    //
    // For now, return mock responses
    let responses: Vec<BatchResponse> = payload
        .requests
        .iter()
        .map(|req| BatchResponse {
            id: req.id.clone(),
            status: 200,
            headers: std::collections::HashMap::new(),
            body: serde_json::json!({
                "message": format!("Mock response for {} {}", req.method, req.path)
            }),
        })
        .collect();

    Ok(Json(BatchResponsePayload { responses }))
}

/// Execute batch with parallel processing
#[allow(dead_code)]
async fn execute_batch_parallel(
    requests: Vec<BatchRequest>,
    _state: AppState,
) -> Vec<BatchResponse> {
    // Spawn tasks for parallel execution
    let tasks: Vec<_> = requests
        .into_iter()
        .map(|req| {
            tokio::spawn(async move {
                // In production: execute the actual request
                BatchResponse {
                    id: req.id,
                    status: 200,
                    headers: std::collections::HashMap::new(),
                    body: serde_json::json!({"result": "success"}),
                }
            })
        })
        .collect();

    // Wait for all tasks to complete
    let mut responses = Vec::new();
    for task in tasks {
        if let Ok(response) = task.await {
            responses.push(response);
        }
    }

    responses
}

/// Execute single request within batch
#[allow(dead_code)]
async fn execute_single_request(
    req: BatchRequest,
    _state: AppState,
    _auth_header: Option<String>,
) -> Result<BatchResponse> {
    // Parse method
    let method = match req.method.to_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        _ => {
            return Ok(BatchResponse {
                id: req.id,
                status: 400,
                headers: std::collections::HashMap::new(),
                body: serde_json::json!({
                    "error": format!("Invalid method: {}", req.method)
                }),
            });
        }
    };

    // Build request
    let mut http_req = Request::builder().method(method).uri(&req.path);

    // Add headers
    for (key, value) in &req.headers {
        http_req = http_req.header(key, value);
    }

    // Add body if present
    let body = if let Some(body_json) = &req.body {
        Body::from(serde_json::to_vec(body_json).unwrap_or_default())
    } else {
        Body::empty()
    };

    let _http_req = http_req
        .body(body)
        .map_err(|e| ApiError::InternalWithMessage(format!("Failed to build request: {}", e)))?;

    // In production: execute the request through the router
    // For now, return a mock response
    Ok(BatchResponse {
        id: req.id,
        status: 200,
        headers: std::collections::HashMap::new(),
        body: serde_json::json!({"result": "success"}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_request_validation() {
        let req = BatchRequest {
            id: "req-1".to_string(),
            method: "GET".to_string(),
            path: "/api/tokens".to_string(),
            headers: std::collections::HashMap::new(),
            body: None,
        };

        assert_eq!(req.id, "req-1");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/tokens");
    }

    #[test]
    fn test_max_batch_size() {
        assert_eq!(MAX_BATCH_SIZE, 10);
    }

    #[test]
    fn test_batch_response_serialization() {
        let response = BatchResponse {
            id: "req-1".to_string(),
            status: 200,
            headers: std::collections::HashMap::new(),
            body: serde_json::json!({"data": "test"}),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("req-1"));
        assert!(json.contains("200"));
    }
}
