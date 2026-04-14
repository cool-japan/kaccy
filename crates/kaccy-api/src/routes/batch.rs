//! Request batching endpoint

use std::collections::HashMap;

use axum::{Json, extract::State};
use futures::future::join_all;
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, Result, ValidationBuilder},
    middleware::Claims,
    state::AppState,
};

/// Maximum number of requests in a batch
const MAX_BATCH_SIZE: usize = 10;

/// Default base URL for sub-requests
const DEFAULT_BASE_URL: &str = "http://localhost:8080";

/// Timeout in seconds for each sub-request
const BATCH_REQUEST_TIMEOUT_SECS: u64 = 30;

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
    pub headers: HashMap<String, String>,
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
    pub headers: HashMap<String, String>,
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
    incoming_headers: axum::http::HeaderMap,
    Json(payload): Json<BatchRequestPayload>,
) -> Result<Json<BatchResponsePayload>> {
    let _user_id = uuid::Uuid::parse_str(&claims.sub)
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

    // Determine base URL from environment
    let base_url = std::env::var("KACCY_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

    // Extract Authorization header from the incoming request to forward
    let auth_header = incoming_headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Build a reqwest client with a per-request timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(BATCH_REQUEST_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Execute all sub-requests in parallel
    let responses =
        execute_batch_parallel(&client, &base_url, &auth_header, &payload.requests).await;

    Ok(Json(BatchResponsePayload { responses }))
}

/// Execute all sub-requests concurrently and return results in order
async fn execute_batch_parallel(
    client: &reqwest::Client,
    base_url: &str,
    auth_header: &Option<String>,
    requests: &[BatchRequest],
) -> Vec<BatchResponse> {
    let futures: Vec<_> = requests
        .iter()
        .map(|req| execute_single_request(client, base_url, auth_header, req))
        .collect();

    join_all(futures).await
}

/// Execute a single sub-request via reqwest and return a `BatchResponse`
async fn execute_single_request(
    client: &reqwest::Client,
    base_url: &str,
    auth_header: &Option<String>,
    request: &BatchRequest,
) -> BatchResponse {
    let url = format!("{}{}", base_url, request.path);

    // Select the appropriate builder for the given HTTP method
    let req_builder = match request.method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => {
            return BatchResponse {
                id: request.id.clone(),
                status: 400,
                headers: HashMap::new(),
                body: serde_json::json!({"error": "unsupported method"}),
            };
        }
    };

    // Forward the Authorization header from the outer request
    let req_builder = if let Some(auth) = auth_header {
        req_builder.header("authorization", auth)
    } else {
        req_builder
    };

    // Forward any extra headers included in the batch request item
    let req_builder = request
        .headers
        .iter()
        .fold(req_builder, |builder, (key, value)| {
            builder.header(key.as_str(), value.as_str())
        });

    // Attach JSON body when provided
    let req_builder = if let Some(body_json) = &request.body {
        req_builder
            .header("content-type", "application/json")
            .json(body_json)
    } else {
        req_builder
    };

    // Execute and map transport-level errors to 502
    let response = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return BatchResponse {
                id: request.id.clone(),
                status: 502,
                headers: HashMap::new(),
                body: serde_json::json!({"error": format!("Sub-request failed: {}", e)}),
            };
        }
    };

    let status = response.status().as_u16();

    // Collect response headers as a plain map
    let resp_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect();

    // Deserialise the body as JSON; fall back to a string wrapper on failure
    let body = match response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => serde_json::json!({"error": format!("Failed to parse response body: {}", e)}),
    };

    BatchResponse {
        id: request.id.clone(),
        status,
        headers: resp_headers,
        body,
    }
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
            headers: HashMap::new(),
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
            headers: HashMap::new(),
            body: serde_json::json!({"data": "test"}),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("req-1"));
        assert!(json.contains("200"));
    }
}
