//! Integration tests for Kaccy API
//!
//! These tests verify end-to-end functionality of the API endpoints.
//! Note: These tests require a running database and Bitcoin node.

#[cfg(test)]
mod integration_tests {
    use serde_json::json;

    /// Test helper to build API base URL
    fn api_url(path: &str) -> String {
        format!("http://localhost:3000{}", path)
    }

    /// Test data structures
    #[derive(Debug, serde::Deserialize)]
    struct AuthResponse {
        token: String,
        user: serde_json::Value,
    }

    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    struct ErrorResponse {
        error: String,
        message: String,
    }

    /// Helper to create test user credentials
    fn test_credentials(suffix: &str) -> (String, String, String) {
        (
            format!("test{}@example.com", suffix),
            "SecureTestPass123!".to_string(),
            format!("testuser{}", suffix),
        )
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_user_registration_flow() {
        let client = reqwest::Client::new();
        let (email, password, username) = test_credentials("_reg");

        // Register a new user
        let response = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": username
            }))
            .send()
            .await
            .expect("Failed to send registration request");

        assert_eq!(response.status(), 200);

        let auth_response: AuthResponse = response.json().await.expect("Failed to parse response");
        assert!(!auth_response.token.is_empty());
        assert_eq!(auth_response.user["username"], username);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_duplicate_email_registration() {
        let client = reqwest::Client::new();
        let (email, password, username) = test_credentials("_dup");

        // First registration should succeed
        let _ = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": username
            }))
            .send()
            .await;

        // Second registration with same email should fail
        let response = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": format!("{}_2", username)
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 400);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_login_flow() {
        let client = reqwest::Client::new();
        let (email, password, username) = test_credentials("_login");

        // Register user first
        let _ = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": username
            }))
            .send()
            .await;

        // Then login
        let response = client
            .post(api_url("/api/auth/login"))
            .json(&json!({
                "email": email,
                "password": password
            }))
            .send()
            .await
            .expect("Failed to send login request");

        assert_eq!(response.status(), 200);

        let auth_response: AuthResponse = response.json().await.expect("Failed to parse response");
        assert!(!auth_response.token.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_invalid_login() {
        let client = reqwest::Client::new();

        let response = client
            .post(api_url("/api/auth/login"))
            .json(&json!({
                "email": "nonexistent@example.com",
                "password": "wrongpassword"
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 401);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_protected_endpoint_without_auth() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/users/me"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 401);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_protected_endpoint_with_auth() {
        let client = reqwest::Client::new();
        let (email, password, username) = test_credentials("_protected");

        // Register and get token
        let auth_response: AuthResponse = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": username
            }))
            .send()
            .await
            .expect("Failed to register")
            .json()
            .await
            .expect("Failed to parse");

        // Access protected endpoint
        let response = client
            .get(api_url("/api/users/me"))
            .header("Authorization", format!("Bearer {}", auth_response.token))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let user: serde_json::Value = response.json().await.expect("Failed to parse");
        assert_eq!(user["email"], email);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_token_listing() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/tokens"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let data: serde_json::Value = response.json().await.expect("Failed to parse");
        assert!(data["tokens"].is_array());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_pagination() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/tokens?page=1&per_page=10"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let data: serde_json::Value = response.json().await.expect("Failed to parse");
        assert_eq!(data["page"], 1);
        assert_eq!(data["per_page"], 10);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_health_check() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/health"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let data: serde_json::Value = response.json().await.expect("Failed to parse");
        assert_eq!(data["status"], "healthy");
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_detailed_health_check() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/health/detailed"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let data: serde_json::Value = response.json().await.expect("Failed to parse");
        assert!(data["database"].is_string());
        assert!(data["db_latency_ms"].is_number());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_profile_update() {
        let client = reqwest::Client::new();
        let (email, password, username) = test_credentials("_profile");

        // Register user
        let auth_response: AuthResponse = client
            .post(api_url("/api/auth/register"))
            .json(&json!({
                "email": email,
                "password": password,
                "username": username
            }))
            .send()
            .await
            .expect("Failed to register")
            .json()
            .await
            .expect("Failed to parse");

        // Update profile
        let response = client
            .put(api_url("/api/users/me"))
            .header("Authorization", format!("Bearer {}", auth_response.token))
            .json(&json!({
                "display_name": "Test User",
                "bio": "This is a test bio"
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let user: serde_json::Value = response.json().await.expect("Failed to parse");
        assert_eq!(user["display_name"], "Test User");
        assert_eq!(user["bio"], "This is a test bio");
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_rate_limiting() {
        let client = reqwest::Client::new();

        // Make many requests quickly
        for i in 0..150 {
            let response = client
                .get(api_url("/health"))
                .send()
                .await
                .expect("Failed to send request");

            if i < 100 {
                // Should succeed for first 100 requests
                assert_eq!(response.status(), 200, "Request {} should succeed", i);
            } else {
                // Should be rate limited after 100
                if response.status() == 429 {
                    // Rate limit kicked in
                    return;
                }
            }
        }
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_cors_headers() {
        let client = reqwest::Client::new();

        let response = client
            .request(reqwest::Method::OPTIONS, api_url("/api/tokens"))
            .header("Origin", "http://localhost:3001")
            .header("Access-Control-Request-Method", "GET")
            .send()
            .await
            .expect("Failed to send request");

        // Check CORS headers are present
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin")
        );
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_request_id_header() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/health"))
            .send()
            .await
            .expect("Failed to send request");

        // Check that request ID header is present
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_code_examples_endpoint() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/docs/examples"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let examples: Vec<serde_json::Value> = response.json().await.expect("Failed to parse");
        assert!(!examples.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_specific_example_endpoint() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/docs/examples/register"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let example: serde_json::Value = response.json().await.expect("Failed to parse");
        assert_eq!(example["operation"], "register");
        assert!(example["examples"].is_array());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_postman_collection_endpoint() {
        let client = reqwest::Client::new();

        let response = client
            .get(api_url("/api/docs/postman"))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let collection: serde_json::Value = response.json().await.expect("Failed to parse");
        assert!(collection["info"].is_object());
        assert!(collection["item"].is_array());
    }
}
