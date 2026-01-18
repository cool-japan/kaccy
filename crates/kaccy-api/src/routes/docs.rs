use crate::examples;
/// Documentation routes for kaccy-api
///
/// Provides API documentation in various formats including Postman collection
use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Postman Collection v2.1 structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    variable: Vec<PostmanVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanInfo {
    name: String,
    description: String,
    version: String,
    schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanItem {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<Vec<PostmanItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<PostmanRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanRequest {
    method: String,
    header: Vec<PostmanHeader>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<PostmanBody>,
    url: PostmanUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanHeader {
    key: String,
    value: String,
    #[serde(rename = "type")]
    header_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanBody {
    mode: String,
    raw: String,
    options: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanUrl {
    raw: String,
    host: Vec<String>,
    path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PostmanVariable {
    key: String,
    value: String,
}

/// Export API as Postman Collection
pub async fn export_postman_collection() -> impl IntoResponse {
    let collection = build_postman_collection();
    (StatusCode::OK, Json(collection))
}

/// Get all code examples
pub async fn get_code_examples() -> impl IntoResponse {
    let examples = examples::get_all_examples();
    (StatusCode::OK, Json(examples))
}

/// Get code examples for a specific operation
pub async fn get_operation_example(
    axum::extract::Path(operation): axum::extract::Path<String>,
) -> impl IntoResponse {
    let examples = examples::get_all_examples();
    match examples.into_iter().find(|e| e.operation == operation) {
        Some(example) => (StatusCode::OK, Json(example)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": "Operation not found"
            })),
        )
            .into_response(),
    }
}

fn build_postman_collection() -> PostmanCollection {
    PostmanCollection {
        info: PostmanInfo {
            name: "Kaccy API".to_string(),
            description: "API for Kaccy Protocol - Personal Token Platform".to_string(),
            version: "1.0.0".to_string(),
            schema: "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
                .to_string(),
        },
        item: vec![
            build_auth_folder(),
            build_users_folder(),
            build_tokens_folder(),
            build_orders_folder(),
            build_trades_folder(),
            build_commitments_folder(),
            build_admin_folder(),
            build_health_folder(),
        ],
        variable: vec![
            PostmanVariable {
                key: "baseUrl".to_string(),
                value: "http://localhost:3000".to_string(),
            },
            PostmanVariable {
                key: "token".to_string(),
                value: "".to_string(),
            },
        ],
    }
}

fn build_auth_folder() -> PostmanItem {
    PostmanItem {
        name: "Authentication".to_string(),
        description: Some("User authentication endpoints".to_string()),
        item: Some(vec![
            PostmanItem {
                name: "Register".to_string(),
                description: Some("Register a new user account".to_string()),
                request: Some(PostmanRequest {
                    method: "POST".to_string(),
                    header: vec![PostmanHeader {
                        key: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        header_type: "text".to_string(),
                    }],
                    body: Some(PostmanBody {
                        mode: "raw".to_string(),
                        raw: json!({
                            "email": "user@example.com",
                            "password": "SecurePassword123!",
                            "username": "johndoe"
                        })
                        .to_string(),
                        options: json!({
                            "raw": {
                                "language": "json"
                            }
                        }),
                    }),
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/api/auth/register".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec![
                            "api".to_string(),
                            "auth".to_string(),
                            "register".to_string(),
                        ],
                    },
                }),
                item: None,
            },
            PostmanItem {
                name: "Login".to_string(),
                description: Some("Login with email and password".to_string()),
                request: Some(PostmanRequest {
                    method: "POST".to_string(),
                    header: vec![PostmanHeader {
                        key: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        header_type: "text".to_string(),
                    }],
                    body: Some(PostmanBody {
                        mode: "raw".to_string(),
                        raw: json!({
                            "email": "user@example.com",
                            "password": "SecurePassword123!"
                        })
                        .to_string(),
                        options: json!({
                            "raw": {
                                "language": "json"
                            }
                        }),
                    }),
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/api/auth/login".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec!["api".to_string(), "auth".to_string(), "login".to_string()],
                    },
                }),
                item: None,
            },
        ]),
        request: None,
    }
}

fn build_users_folder() -> PostmanItem {
    PostmanItem {
        name: "Users".to_string(),
        description: Some("User profile management endpoints".to_string()),
        item: Some(vec![PostmanItem {
            name: "Get My Profile".to_string(),
            description: Some("Get current user's profile".to_string()),
            request: Some(PostmanRequest {
                method: "GET".to_string(),
                header: vec![PostmanHeader {
                    key: "Authorization".to_string(),
                    value: "Bearer {{token}}".to_string(),
                    header_type: "text".to_string(),
                }],
                body: None,
                url: PostmanUrl {
                    raw: "{{baseUrl}}/api/users/me".to_string(),
                    host: vec!["{{baseUrl}}".to_string()],
                    path: vec!["api".to_string(), "users".to_string(), "me".to_string()],
                },
            }),
            item: None,
        }]),
        request: None,
    }
}

fn build_tokens_folder() -> PostmanItem {
    PostmanItem {
        name: "Tokens".to_string(),
        description: Some("Token management endpoints".to_string()),
        item: Some(vec![
            PostmanItem {
                name: "List Tokens".to_string(),
                description: Some("Get list of all tokens with pagination".to_string()),
                request: Some(PostmanRequest {
                    method: "GET".to_string(),
                    header: vec![],
                    body: None,
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/api/tokens?page=1&limit=20".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec!["api".to_string(), "tokens".to_string()],
                    },
                }),
                item: None,
            },
            PostmanItem {
                name: "Create Token".to_string(),
                description: Some("Create a new personal token".to_string()),
                request: Some(PostmanRequest {
                    method: "POST".to_string(),
                    header: vec![
                        PostmanHeader {
                            key: "Authorization".to_string(),
                            value: "Bearer {{token}}".to_string(),
                            header_type: "text".to_string(),
                        },
                        PostmanHeader {
                            key: "Content-Type".to_string(),
                            value: "application/json".to_string(),
                            header_type: "text".to_string(),
                        },
                    ],
                    body: Some(PostmanBody {
                        mode: "raw".to_string(),
                        raw: json!({
                            "symbol": "MYTOKEN",
                            "name": "My Personal Token",
                            "initial_supply": "1000000",
                            "initial_price_btc": "0.00001"
                        })
                        .to_string(),
                        options: json!({
                            "raw": {
                                "language": "json"
                            }
                        }),
                    }),
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/api/tokens".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec!["api".to_string(), "tokens".to_string()],
                    },
                }),
                item: None,
            },
        ]),
        request: None,
    }
}

fn build_orders_folder() -> PostmanItem {
    PostmanItem {
        name: "Orders".to_string(),
        description: Some("Order management endpoints".to_string()),
        item: Some(vec![PostmanItem {
            name: "Buy Order".to_string(),
            description: Some("Create a buy order for a token".to_string()),
            request: Some(PostmanRequest {
                method: "POST".to_string(),
                header: vec![
                    PostmanHeader {
                        key: "Authorization".to_string(),
                        value: "Bearer {{token}}".to_string(),
                        header_type: "text".to_string(),
                    },
                    PostmanHeader {
                        key: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        header_type: "text".to_string(),
                    },
                ],
                body: Some(PostmanBody {
                    mode: "raw".to_string(),
                    raw: json!({
                        "token_id": "00000000-0000-0000-0000-000000000000",
                        "amount": "100"
                    })
                    .to_string(),
                    options: json!({
                        "raw": {
                            "language": "json"
                        }
                    }),
                }),
                url: PostmanUrl {
                    raw: "{{baseUrl}}/api/orders/buy".to_string(),
                    host: vec!["{{baseUrl}}".to_string()],
                    path: vec!["api".to_string(), "orders".to_string(), "buy".to_string()],
                },
            }),
            item: None,
        }]),
        request: None,
    }
}

fn build_trades_folder() -> PostmanItem {
    PostmanItem {
        name: "Trades".to_string(),
        description: Some("Trade history endpoints".to_string()),
        item: Some(vec![PostmanItem {
            name: "Get My Trades".to_string(),
            description: Some("Get current user's trade history".to_string()),
            request: Some(PostmanRequest {
                method: "GET".to_string(),
                header: vec![PostmanHeader {
                    key: "Authorization".to_string(),
                    value: "Bearer {{token}}".to_string(),
                    header_type: "text".to_string(),
                }],
                body: None,
                url: PostmanUrl {
                    raw: "{{baseUrl}}/api/trades".to_string(),
                    host: vec!["{{baseUrl}}".to_string()],
                    path: vec!["api".to_string(), "trades".to_string()],
                },
            }),
            item: None,
        }]),
        request: None,
    }
}

fn build_commitments_folder() -> PostmanItem {
    PostmanItem {
        name: "Commitments".to_string(),
        description: Some("Commitment management endpoints".to_string()),
        item: Some(vec![PostmanItem {
            name: "Create Commitment".to_string(),
            description: Some("Create a new commitment".to_string()),
            request: Some(PostmanRequest {
                method: "POST".to_string(),
                header: vec![
                    PostmanHeader {
                        key: "Authorization".to_string(),
                        value: "Bearer {{token}}".to_string(),
                        header_type: "text".to_string(),
                    },
                    PostmanHeader {
                        key: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        header_type: "text".to_string(),
                    },
                ],
                body: Some(PostmanBody {
                    mode: "raw".to_string(),
                    raw: json!({
                        "token_id": "00000000-0000-0000-0000-000000000000",
                        "title": "Complete Project",
                        "description": "I will complete this project by the deadline",
                        "deadline": "2026-12-31T23:59:59Z"
                    })
                    .to_string(),
                    options: json!({
                        "raw": {
                            "language": "json"
                        }
                    }),
                }),
                url: PostmanUrl {
                    raw: "{{baseUrl}}/api/commitments".to_string(),
                    host: vec!["{{baseUrl}}".to_string()],
                    path: vec!["api".to_string(), "commitments".to_string()],
                },
            }),
            item: None,
        }]),
        request: None,
    }
}

fn build_admin_folder() -> PostmanItem {
    PostmanItem {
        name: "Admin".to_string(),
        description: Some("Admin-only endpoints".to_string()),
        item: Some(vec![PostmanItem {
            name: "Get Stats".to_string(),
            description: Some("Get platform statistics".to_string()),
            request: Some(PostmanRequest {
                method: "GET".to_string(),
                header: vec![PostmanHeader {
                    key: "Authorization".to_string(),
                    value: "Bearer {{token}}".to_string(),
                    header_type: "text".to_string(),
                }],
                body: None,
                url: PostmanUrl {
                    raw: "{{baseUrl}}/api/admin/stats".to_string(),
                    host: vec!["{{baseUrl}}".to_string()],
                    path: vec!["api".to_string(), "admin".to_string(), "stats".to_string()],
                },
            }),
            item: None,
        }]),
        request: None,
    }
}

fn build_health_folder() -> PostmanItem {
    PostmanItem {
        name: "Health".to_string(),
        description: Some("Health check endpoints".to_string()),
        item: Some(vec![
            PostmanItem {
                name: "Basic Health Check".to_string(),
                description: Some("Check if the API is running".to_string()),
                request: Some(PostmanRequest {
                    method: "GET".to_string(),
                    header: vec![],
                    body: None,
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/health".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec!["health".to_string()],
                    },
                }),
                item: None,
            },
            PostmanItem {
                name: "Detailed Health Check".to_string(),
                description: Some("Get detailed health information".to_string()),
                request: Some(PostmanRequest {
                    method: "GET".to_string(),
                    header: vec![],
                    body: None,
                    url: PostmanUrl {
                        raw: "{{baseUrl}}/health/detailed".to_string(),
                        host: vec!["{{baseUrl}}".to_string()],
                        path: vec!["health".to_string(), "detailed".to_string()],
                    },
                }),
                item: None,
            },
        ]),
        request: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postman_collection_structure() {
        let collection = build_postman_collection();
        assert_eq!(collection.info.name, "Kaccy API");
        assert_eq!(collection.variable.len(), 2);
        assert!(!collection.item.is_empty());
    }

    #[test]
    fn test_auth_folder() {
        let folder = build_auth_folder();
        assert_eq!(folder.name, "Authentication");
        assert!(folder.item.is_some());
        let items = folder.item.unwrap();
        assert_eq!(items.len(), 2); // Register and Login
    }

    #[test]
    fn test_health_folder() {
        let folder = build_health_folder();
        assert_eq!(folder.name, "Health");
        assert!(folder.item.is_some());
        let items = folder.item.unwrap();
        assert_eq!(items.len(), 2); // Basic and Detailed health
    }
}
