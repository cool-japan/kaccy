//! Kaccy API Server

use axum::{
    Extension, Router, middleware,
    routing::{delete, get, post, put},
};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa_swagger_ui::SwaggerUi;

use kaccy_api::{
    config::Config,
    events::{
        EventBus, EventPublisher, PublisherConfig,
        handlers::{
            AnalyticsHandler, AuditLogHandler, MetricsHandler, NotificationHandler, WebhookHandler,
        },
    },
    jobs::{JobProcessor, JobQueue, JobQueueConfig, JobScheduler},
    middleware::{
        RequestValidationConfig, SecurityHeadersConfig, TracingConfig,
        auth::{require_admin, require_auth},
        cache_middleware, init_metrics, init_telemetry, logging_middleware, metrics_handler,
        metrics_middleware, profiling_middleware, request_id_middleware,
        request_validation_middleware, security_headers_middleware, shutdown_telemetry,
        tracing_middleware, version_middleware,
    },
    openapi::ApiDoc,
    routes,
    state::AppState,
};
use kaccy_db::create_pool;
use utoipa::OpenApi;

/// Test optional service connections and add them to state if configured
async fn test_optional_services(state: &mut AppState, config: &Config) {
    // Test Redis connection if configured
    if let Some(redis_url) = &config.integrations.redis_url {
        match kaccy_db::RedisCache::new(kaccy_db::CacheConfig {
            redis_url: redis_url.clone(),
            key_prefix: "kaccy:".to_string(),
            default_ttl_secs: 300,
            session_ttl_secs: 3600,
            price_ttl_secs: 60,
            profile_ttl_secs: 600,
            balance_ttl_secs: 300,
        })
        .await
        {
            Ok(cache) => {
                // Test Redis with a ping
                match cache.set("_health_check", &"ok", 60).await {
                    Ok(_) => {
                        *state = state.clone().with_redis(cache);
                        tracing::info!("✓ Redis connection verified and configured");
                    }
                    Err(e) => {
                        tracing::warn!("⚠ Redis configured but connection test failed: {}", e);
                        tracing::warn!(
                            "  Continuing without Redis (cache middleware will be disabled)"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!("⚠ Redis configured but initialization failed: {}", e);
                tracing::warn!("  Continuing without Redis (cache middleware will be disabled)");
            }
        }
    } else {
        tracing::info!("○ Redis not configured (cache middleware will be disabled)");
    }

    // Bitcoin RPC configuration status (detailed setup would require URL parsing)
    if config.integrations.bitcoin_rpc_url.is_some() {
        tracing::info!("✓ Bitcoin RPC configured (will be used for payment processing)");
    } else {
        tracing::info!("○ Bitcoin RPC not configured (payment processing will be limited)");
    }

    // Log SMTP configuration status
    if config.integrations.smtp.is_some() {
        tracing::info!("✓ SMTP email configured");
    } else {
        tracing::info!("○ SMTP not configured (email notifications will use stub)");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load and validate configuration
    let config = Config::from_env().map_err(|e| {
        eprintln!("Configuration error: {}", e);
        e
    })?;

    config.validate().map_err(|e| {
        eprintln!("Configuration validation error: {}", e);
        e
    })?;

    // Initialize OpenTelemetry (optional - only if OTEL endpoint is configured)
    let _telemetry_provider = if config.observability.enable_telemetry {
        match init_telemetry(TracingConfig::default()) {
            Ok(provider) => {
                tracing::info!("OpenTelemetry initialized");
                Some(provider)
            }
            Err(e) => {
                eprintln!("Failed to initialize OpenTelemetry: {}", e);
                None
            }
        }
    } else {
        tracing::info!("OpenTelemetry disabled (OTEL_EXPORTER_OTLP_ENDPOINT not set)");
        None
    };

    // Initialize tracing with OpenTelemetry layer if available
    let telemetry_layer = _telemetry_provider
        .as_ref()
        .map(|_| tracing_opentelemetry::layer());

    if let Some(layer) = telemetry_layer {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| config.observability.log_level.clone().into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| config.observability.log_level.clone().into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    // Display startup banner
    display_startup_banner(&config);

    // Initialize Prometheus metrics
    if config.observability.enable_metrics {
        init_metrics();
        tracing::info!("Prometheus metrics initialized");
    }

    // Create database pool
    let pool = create_pool(&config.database.url).await?;
    tracing::info!("✓ Database connection pool created");

    // Test database connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Database connection test failed: {}", e))?;
    tracing::info!("✓ Database connection verified");

    // Initialize event bus with all handlers
    let event_publisher = EventPublisher::new(PublisherConfig::default());
    let event_bus = EventBus::new(event_publisher);

    // Register event handlers
    event_bus
        .register_handler(std::sync::Arc::new(AuditLogHandler::new(Some(
            std::sync::Arc::new(pool.clone()),
        ))))
        .await;
    event_bus
        .register_handler(std::sync::Arc::new(MetricsHandler))
        .await;
    event_bus
        .register_handler(std::sync::Arc::new(NotificationHandler))
        .await;
    event_bus
        .register_handler(std::sync::Arc::new(WebhookHandler))
        .await;
    event_bus
        .register_handler(std::sync::Arc::new(AnalyticsHandler))
        .await;

    tracing::info!("Event bus initialized with 5 handlers");

    // Initialize job processor
    let job_queue_config = JobQueueConfig {
        max_queue_size: config.jobs.max_queue_size,
        enable_persistence: config.jobs.enable_persistence,
    };
    let job_queue = JobQueue::new(job_queue_config);
    let job_scheduler = JobScheduler::new();

    let mut job_processor = JobProcessor::new(job_queue, job_scheduler, config.jobs.worker_count);
    job_processor.start().await?;

    tracing::info!(
        "Job processor started with {} workers",
        config.jobs.worker_count
    );

    // Create app state with event bus and job processor
    let mut state = AppState::new(pool.clone(), config.security.jwt_secret.clone())
        .with_event_bus(event_bus)
        .with_job_processor(job_processor);

    // Test optional service connections
    test_optional_services(&mut state, &config).await;

    // Create GraphQL schema
    let graphql_schema = routes::graphql::create_schema(pool.clone());

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    let public_routes = Router::new()
        // Health endpoints
        .route("/health", get(routes::health::health))
        .route("/health/detailed", get(routes::health::health_detailed))
        .route("/ready", get(routes::health::ready))
        // Metrics endpoint
        .route("/metrics", get(metrics_handler))
        // Auth endpoints
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login))
        // Public token endpoints
        .route("/api/tokens", get(routes::tokens::list_tokens))
        .route("/api/tokens/:token_id", get(routes::tokens::get_token))
        .route(
            "/api/tokens/:token_id/trades",
            get(routes::trades::get_token_trades),
        )
        .route(
            "/api/tokens/:token_id/holders",
            get(routes::trades::get_token_holders),
        )
        .route(
            "/api/tokens/:token_id/commitments",
            get(routes::commitments::get_token_commitments),
        )
        // Public user endpoints
        .route("/api/users/:user_id", get(routes::users::get_user))
        .route(
            "/api/users/:user_id/tokens",
            get(routes::users::get_user_tokens),
        )
        // OAuth2 endpoints
        .route("/api/oauth2/login", get(routes::oauth2::oauth2_login))
        .route("/api/oauth2/callback", get(routes::oauth2::oauth2_callback))
        // GraphQL endpoints
        .route("/api/graphql", post(routes::graphql::graphql_handler))
        .route(
            "/api/graphql/playground",
            get(routes::graphql::graphql_playground),
        )
        // Batch API endpoint
        .route("/api/batch", post(routes::batch::execute_batch))
        // Streaming endpoints
        .route(
            "/api/stream/tokens/ndjson",
            get(routes::streaming::stream_tokens_ndjson),
        )
        .route(
            "/api/stream/orders/jsonlines",
            get(routes::streaming::stream_orders_jsonlines),
        )
        .route(
            "/api/stream/tokens/json",
            get(routes::streaming::stream_tokens_chunked_json),
        )
        .route(
            "/api/stream/tokens/csv",
            get(routes::streaming::export_tokens_csv),
        )
        // Documentation endpoints
        .route(
            "/api/docs/postman",
            get(routes::docs::export_postman_collection),
        )
        .route("/api/docs/examples", get(routes::docs::get_code_examples))
        .route(
            "/api/docs/examples/:operation_id",
            get(routes::docs::get_operation_example),
        )
        // Pool stats endpoints
        .route("/api/pool/stats", get(routes::pool_stats::get_pool_stats))
        .route(
            "/api/pool/recommendations",
            get(routes::pool_stats::get_pool_recommendations),
        )
        // WebSocket endpoints
        .route(
            "/ws/tokens/:token_id/prices",
            get(routes::ws::token_prices_ws),
        )
        .route("/ws/tokens/:token_id", get(routes::ws::token_ws))
        .route("/ws/users/orders", get(routes::ws::user_orders_ws))
        .route("/ws/tokens/:token_id/chat", get(routes::ws::token_chat_ws))
        // Marketing unsubscribe (public, no auth)
        .route(
            "/api/notifications/unsubscribe/:user_id",
            get(routes::notifications::unsubscribe_marketing),
        );

    // Protected routes (auth required)
    let protected_routes = Router::new()
        // User profile
        .route("/api/users/me", get(routes::users::get_me))
        .route("/api/users/me", put(routes::users::update_me))
        .route("/api/portfolio", get(routes::users::get_portfolio))
        .route("/api/trades", get(routes::users::get_my_trades))
        // Token creation
        .route("/api/tokens", post(routes::tokens::create_token))
        // Order management
        .route("/api/orders", get(routes::orders::get_my_orders))
        .route("/api/orders/buy", post(routes::orders::create_buy_order))
        .route("/api/orders/sell", post(routes::orders::create_sell_order))
        .route("/api/orders/:order_id", get(routes::orders::get_order))
        .route(
            "/api/orders/:order_id",
            delete(routes::orders::cancel_order),
        )
        // Commitment management
        .route(
            "/api/commitments",
            post(routes::commitments::create_commitment),
        )
        .route(
            "/api/commitments",
            get(routes::commitments::get_my_commitments),
        )
        .route(
            "/api/commitments/:commitment_id",
            get(routes::commitments::get_commitment),
        )
        .route(
            "/api/commitments/:commitment_id/evidence",
            post(routes::commitments::submit_evidence),
        )
        // API key management
        .route("/api/api-keys", post(routes::api_keys::create_api_key))
        .route("/api/api-keys", get(routes::api_keys::list_api_keys))
        .route("/api/api-keys/:id", get(routes::api_keys::get_api_key))
        .route("/api/api-keys/:id", put(routes::api_keys::update_api_key))
        .route(
            "/api/api-keys/:id/revoke",
            post(routes::api_keys::revoke_api_key),
        )
        .route(
            "/api/api-keys/:id",
            delete(routes::api_keys::delete_api_key),
        )
        .route(
            "/api/api-keys/:id/rotate",
            post(routes::api_keys::rotate_api_key),
        )
        // API key analytics
        .route(
            "/api/api-keys/:id/usage",
            get(routes::api_key_analytics::get_api_key_usage),
        )
        .route(
            "/api/api-keys/:id/usage/logs",
            get(routes::api_key_analytics::get_api_key_usage_logs),
        )
        .route(
            "/api/api-keys/:id/quota",
            get(routes::api_key_analytics::get_api_key_quota),
        )
        .route(
            "/api/api-keys/usage/all",
            get(routes::api_key_analytics::get_all_api_keys_usage),
        )
        // KYC management
        .route("/api/kyc", get(routes::kyc::get_my_kyc))
        .route("/api/kyc", post(routes::kyc::submit_kyc))
        // Webhook management
        .route("/api/webhooks", post(routes::webhooks::create_webhook))
        .route("/api/webhooks", get(routes::webhooks::list_webhooks))
        .route("/api/webhooks/:id", get(routes::webhooks::get_webhook))
        .route("/api/webhooks/:id", put(routes::webhooks::update_webhook))
        .route(
            "/api/webhooks/:id",
            delete(routes::webhooks::delete_webhook),
        )
        .route(
            "/api/webhooks/:id/deliveries",
            get(routes::webhooks::list_webhook_deliveries),
        )
        // 2FA/TOTP management
        .route("/api/2fa/setup", post(routes::totp::setup_2fa))
        .route("/api/2fa/enable", post(routes::totp::enable_2fa))
        .route("/api/2fa/disable", post(routes::totp::disable_2fa))
        .route("/api/2fa/status", get(routes::totp::get_2fa_status))
        .route("/api/2fa/verify", post(routes::totp::verify_2fa_code))
        // Notification preferences
        .route(
            "/api/notifications/preferences",
            get(routes::notifications::get_notification_preferences),
        )
        .route(
            "/api/notifications/preferences",
            put(routes::notifications::update_notification_preferences),
        )
        // Session management (user routes)
        .route("/api/sessions/my", get(routes::sessions::get_my_sessions))
        .route(
            "/api/sessions/my/history",
            get(routes::sessions::get_my_session_history),
        )
        .route(
            "/api/sessions/:session_id",
            delete(routes::sessions::logout_session),
        )
        .route(
            "/api/sessions/logout-others",
            post(routes::sessions::logout_other_sessions),
        )
        .route(
            "/api/sessions/logout-all",
            post(routes::sessions::logout_all_sessions),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Admin routes
    let admin_routes = Router::new()
        .route("/api/admin/stats", get(routes::admin::get_admin_stats))
        .route("/api/admin/users", get(routes::admin::list_users))
        .route(
            "/api/admin/users/:user_id/role",
            put(routes::admin::update_user_role),
        )
        .route(
            "/api/admin/commitments/pending",
            get(routes::admin::get_pending_commitments),
        )
        .route(
            "/api/admin/commitments/:commitment_id/verify",
            post(routes::admin::verify_commitment),
        )
        // KYC admin routes
        .route("/api/admin/kyc-queue", get(routes::kyc::get_kyc_queue))
        .route("/api/admin/kyc/:id/verify", post(routes::kyc::verify_kyc))
        .route("/api/admin/kyc/stats", get(routes::kyc::get_kyc_stats))
        // Rate limit admin routes
        .route(
            "/api/admin/rate-limits",
            get(routes::rate_limit_admin::get_rate_limit_status_app),
        )
        .route(
            "/api/admin/rate-limits/adjust",
            post(routes::rate_limit_admin::adjust_rate_limit_multiplier_app),
        )
        .route(
            "/api/admin/rate-limits/reset",
            post(routes::rate_limit_admin::reset_rate_limit_app),
        )
        // Audit log routes
        .route(
            "/api/admin/audit-events",
            get(routes::audit::list_audit_events),
        )
        .route(
            "/api/admin/audit-events/:event_id",
            get(routes::audit::get_audit_event),
        )
        .route(
            "/api/admin/audit-events/user/:user_id",
            get(routes::audit::get_user_audit_events),
        )
        .route(
            "/api/admin/audit-events/stats",
            get(routes::audit::get_audit_stats),
        )
        // System metrics routes
        .route(
            "/api/admin/metrics",
            get(routes::metrics::get_system_metrics),
        )
        .route(
            "/api/admin/health/comprehensive",
            get(routes::metrics::get_comprehensive_health),
        )
        // Background jobs routes
        .route("/api/admin/jobs", get(routes::background_jobs::list_jobs))
        .route(
            "/api/admin/jobs/stats",
            get(routes::background_jobs::get_job_stats),
        )
        .route(
            "/api/admin/jobs/stuck",
            get(routes::background_jobs::get_stuck_jobs),
        )
        .route(
            "/api/admin/jobs/stuck/reset",
            post(routes::background_jobs::reset_stuck_jobs),
        )
        .route(
            "/api/admin/jobs/cleanup",
            delete(routes::background_jobs::cleanup_old_jobs),
        )
        .route("/api/admin/jobs", post(routes::background_jobs::create_job))
        .route(
            "/api/admin/jobs/:job_id",
            get(routes::background_jobs::get_job),
        )
        .route(
            "/api/admin/jobs/:job_id/retry",
            post(routes::background_jobs::retry_job),
        )
        .route(
            "/api/admin/jobs/:job_id/cancel",
            post(routes::background_jobs::cancel_job),
        )
        // Session management (admin routes)
        .route(
            "/api/admin/sessions/:user_id/count",
            get(routes::sessions::get_active_session_count),
        )
        .route(
            "/api/admin/sessions/:user_id/suspicious",
            get(routes::sessions::check_suspicious_logins),
        )
        .route(
            "/api/admin/sessions/:session_id/revoke",
            post(routes::sessions::revoke_session_admin),
        )
        .route(
            "/api/admin/sessions/by-ip/:ip_address",
            get(routes::sessions::get_sessions_by_ip),
        )
        .route(
            "/api/admin/sessions/:user_id/by-device/:device_type",
            get(routes::sessions::get_sessions_by_device),
        )
        .route(
            "/api/admin/sessions/cleanup/expired",
            post(routes::sessions::cleanup_expired_sessions),
        )
        .route(
            "/api/admin/sessions/cleanup/old",
            post(routes::sessions::cleanup_old_sessions),
        )
        // System configuration routes
        .route(
            "/api/admin/config",
            get(routes::system_config::list_configs),
        )
        .route(
            "/api/admin/config/summary",
            get(routes::system_config::get_category_summary),
        )
        .route(
            "/api/admin/config/count",
            get(routes::system_config::get_active_config_count),
        )
        .route(
            "/api/admin/config",
            post(routes::system_config::create_config),
        )
        .route(
            "/api/admin/config/batch",
            post(routes::system_config::batch_update_configs),
        )
        .route(
            "/api/admin/config/:config_key",
            get(routes::system_config::get_config),
        )
        .route(
            "/api/admin/config/:config_key",
            put(routes::system_config::update_config),
        )
        .route(
            "/api/admin/config/:config_key",
            delete(routes::system_config::delete_config),
        )
        .route(
            "/api/admin/config/:config_key/history",
            get(routes::system_config::get_config_history),
        )
        .route(
            "/api/admin/config/:config_key/versions",
            get(routes::system_config::get_config_versions),
        )
        .route(
            "/api/admin/config/:config_key/rollback",
            post(routes::system_config::rollback_config),
        )
        .route(
            "/api/admin/config/feature/:feature_key",
            get(routes::system_config::is_feature_enabled),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_admin));

    // Clone state for cleanup before moving into router
    let state_for_cleanup = state.clone();

    // Configure request validation based on environment
    let request_validation_config = match config.server.environment {
        kaccy_api::config::Environment::Production => RequestValidationConfig::strict(),
        kaccy_api::config::Environment::Staging => RequestValidationConfig::default(),
        kaccy_api::config::Environment::Development => RequestValidationConfig::relaxed(),
    };

    // Configure security headers based on environment
    let security_headers_config = match config.server.environment {
        kaccy_api::config::Environment::Production => SecurityHeadersConfig::api(),
        kaccy_api::config::Environment::Staging => SecurityHeadersConfig::production(),
        kaccy_api::config::Environment::Development => SecurityHeadersConfig::development(),
    };

    // Clone state for middleware that needs it
    let state_for_middleware = state.clone();

    tracing::info!("Initializing middleware stack...");
    tracing::debug!("  - Request ID middleware");
    tracing::debug!("  - Distributed tracing middleware");
    tracing::debug!("  - API versioning middleware");
    tracing::debug!("  - Prometheus metrics middleware");
    tracing::debug!("  - Performance profiling middleware");
    tracing::debug!("  - Structured logging middleware");
    if state_for_middleware.has_redis() {
        tracing::debug!("  - Redis cache middleware (active)");
    } else {
        tracing::debug!("  - Redis cache middleware (inactive - no Redis)");
    }
    tracing::debug!("  - Request validation middleware");
    tracing::debug!("  - Security headers middleware");
    tracing::debug!("  - CORS layer");
    tracing::debug!("  - Compression layer (gzip/brotli)");
    tracing::info!("✓ Middleware stack initialized");

    // Combine all routes with middleware layers
    // Note: Layers are applied in reverse order (bottom to top)
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_routes)
        .with_state(state)
        // OpenAPI documentation (stateless routes)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(Extension(graphql_schema))
        .layer(Extension(security_headers_config))
        .layer(Extension(request_validation_config))
        .layer(CompressionLayer::new())
        .layer(cors)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn(request_validation_middleware))
        .layer(middleware::from_fn_with_state(
            state_for_middleware,
            cache_middleware,
        )) // Redis-based HTTP response caching for GET requests
        .layer(middleware::from_fn(logging_middleware))
        .layer(middleware::from_fn(profiling_middleware)) // Performance profiling with response time header
        .layer(middleware::from_fn(metrics_middleware))
        .layer(middleware::from_fn(version_middleware)) // API version header injection
        .layer(middleware::from_fn(tracing_middleware))
        .layer(middleware::from_fn(request_id_middleware));

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("🚀 Server listening on {}", addr);

    // Set up graceful shutdown handler
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
        tracing::info!("Shutdown signal received, initiating graceful shutdown...");
    };
    let telemetry_provider = _telemetry_provider.clone();

    // Run server with graceful shutdown
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await;

    // Perform cleanup after server stops accepting new connections
    tracing::info!("Server stopped accepting new connections, performing cleanup...");

    // Stop job processor gracefully
    if let Some(job_processor) = &state_for_cleanup.job_processor {
        if let Ok(mut processor) = job_processor.try_lock() {
            if let Err(e) = processor.stop().await {
                tracing::error!("Error stopping job processor: {}", e);
            } else {
                tracing::info!("Job processor stopped gracefully");
            }
        } else {
            tracing::warn!("Job processor is locked, waiting for completion...");
            let mut processor = job_processor.lock().await;
            if let Err(e) = processor.stop().await {
                tracing::error!("Error stopping job processor: {}", e);
            } else {
                tracing::info!("Job processor stopped gracefully");
            }
        }
    }

    // Close database pool
    state_for_cleanup.pool.close().await;
    tracing::info!("Database connection pool closed");

    // Close Redis connection if configured
    if let Some(redis) = state_for_cleanup.redis_cache {
        drop(redis);
        tracing::info!("Redis connection closed");
    }

    // Close Bitcoin client if configured
    if let Some(bitcoin) = state_for_cleanup.bitcoin_client {
        drop(bitcoin);
        tracing::info!("Bitcoin client closed");
    }

    // Shutdown OpenTelemetry gracefully
    if telemetry_provider.is_some() {
        shutdown_telemetry();
        tracing::info!("OpenTelemetry shutdown complete");
    }

    tracing::info!("Graceful shutdown complete");

    server?;
    Ok(())
}

/// Display startup banner with configuration summary
fn display_startup_banner(config: &Config) {
    println!("\n{}", "=".repeat(70));
    println!("  🚀 Kaccy API Server");
    println!("{}", "=".repeat(70));
    println!("  Environment:       {}", config.server.environment);
    println!(
        "  Server:            {}:{}",
        config.server.host, config.server.port
    );
    println!(
        "  Database:          {} (max_conn: {})",
        mask_database_url(&config.database.url),
        config.database.max_connections
    );
    println!("  Job Workers:       {}", config.jobs.worker_count);
    println!("  Job Queue Size:    {}", config.jobs.max_queue_size);
    println!(
        "  Metrics:           {}",
        if config.observability.enable_metrics {
            "✓ enabled"
        } else {
            "✗ disabled"
        }
    );
    println!(
        "  Telemetry:         {}",
        if config.observability.enable_telemetry {
            "✓ enabled"
        } else {
            "✗ disabled"
        }
    );
    println!("  Log Level:         {}", config.observability.log_level);

    // Optional integrations
    println!("\n  Optional Integrations:");
    println!(
        "    Redis Cache:     {}",
        if config.integrations.redis_url.is_some() {
            "✓ configured"
        } else {
            "✗ not configured"
        }
    );
    println!(
        "    Bitcoin RPC:     {}",
        if config.integrations.bitcoin_rpc_url.is_some() {
            "✓ configured"
        } else {
            "✗ not configured"
        }
    );
    println!(
        "    SMTP Email:      {}",
        if config.integrations.smtp.is_some() {
            "✓ configured"
        } else {
            "✗ not configured"
        }
    );

    // Middleware stack
    println!("\n  Active Middleware:");
    println!("    Request ID       ✓");
    println!("    Tracing          ✓");
    println!("    Versioning       ✓ (API version detection)");
    println!("    Metrics          ✓ (Prometheus)");
    println!("    Profiling        ✓ (Response time tracking)");
    println!("    Logging          ✓ (Structured logs)");
    println!(
        "    Cache            {}",
        if config.integrations.redis_url.is_some() {
            "✓ (Redis)"
        } else {
            "○ (No Redis)"
        }
    );
    println!("    Validation       ✓ (Request validation)");
    println!("    Security         ✓ (Security headers)");
    println!("    CORS             ✓");
    println!("    Compression      ✓ (gzip/br)");

    println!("{}", "=".repeat(70));
    println!();
}

/// Mask sensitive parts of database URL
fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.rfind('@') {
        if let Some(protocol_end) = url.find("://") {
            let protocol = &url[..protocol_end + 3];
            let after_at = &url[at_pos..];
            format!("{}***{}", protocol, after_at)
        } else {
            "***".to_string()
        }
    } else {
        url.to_string()
    }
}
