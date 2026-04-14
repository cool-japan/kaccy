//! API layer for Kaccy Protocol
//!
//! This crate provides the REST API endpoints using Axum framework.
//!
//! # Features
//!
//! - **Authentication & Authorization**: JWT-based auth, OAuth2, 2FA/TOTP, API keys
//! - **REST API**: Comprehensive endpoints for users, tokens, orders, trades, commitments
//! - **GraphQL**: Alternative GraphQL endpoint with playground
//! - **WebSocket**: Real-time updates for prices, trades, orders, and chat
//! - **Notifications**: Email (SMTP), SMS (Twilio/AWS SNS), Push (FCM/APNS), In-app
//! - **Background Jobs**: Asynchronous job processing with retry logic
//! - **Event System**: Event-driven architecture with audit logging and webhooks
//! - **Observability**: Prometheus metrics, OpenTelemetry tracing, structured logging
//! - **Security**: Rate limiting, request signing, IP filtering, CORS
//! - **Performance**: Response caching (Redis), request batching, streaming, circuit breaker
//! - **Documentation**: OpenAPI/Swagger, Postman collections, SDK generation
//!
//! # Module Overview
//!
//! - [`analytics`]: User behavior tracking, performance metrics, business intelligence
//! - [`config`]: Configuration management with validation and environment support
//! - [`error`]: Error types and standardized error responses
//! - [`events`]: Event-driven architecture with handlers for audit, metrics, notifications, webhooks
//! - [`examples`]: Code examples for API operations in multiple languages
//! - [`jobs`]: Background job processing system with worker pools
//! - [`middleware`]: Authentication, authorization, rate limiting, logging, tracing, metrics
//! - [`notifications`]: Multi-channel notification system (email, SMS, push, in-app)
//! - [`openapi`]: OpenAPI/Swagger documentation generation
//! - [`routes`]: HTTP route handlers for all API endpoints
//! - [`sdk_generator`]: Client SDK generation for TypeScript, Python, Rust, Go
//! - [`state`]: Application state management with database pool, Redis, Bitcoin client

/// User behavior tracking, performance metrics, business intelligence
pub mod analytics;
/// Configuration management with validation and environment support
pub mod config;
/// Error types and standardized error responses
pub mod error;
/// Event-driven architecture with handlers for audit, metrics, notifications, webhooks
pub mod events;
/// Code examples for API operations in multiple languages
pub mod examples;
/// Background job processing system with worker pools
pub mod jobs;
/// Authentication, authorization, rate limiting, logging, tracing, metrics
pub mod middleware;
/// Multi-channel notification system (email, SMS, push, in-app)
pub mod notifications;
/// OpenAPI/Swagger documentation generation
pub mod openapi;
/// HTTP route handlers for all API endpoints
pub mod routes;
/// Client SDK generation for TypeScript, Python, Rust, Go
pub mod sdk_generator;
/// Application state management with database pool, Redis, Bitcoin client
pub mod state;
