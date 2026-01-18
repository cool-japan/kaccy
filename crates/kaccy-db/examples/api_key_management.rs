//! API Key Management Example
//!
//! This example demonstrates how to use the API key repositories for:
//! - Creating and managing API keys
//! - Validating API keys
//! - Tracking API usage
//! - Analyzing usage statistics
//! - Managing key lifecycle (revocation, expiration)
//!
//! Note: This example uses runtime queries and does not require DATABASE_URL to compile.

use chrono::{Duration, Utc};
use kaccy_db::{create_pool, ApiKeyRepository, ApiKeyUsageRepository, CreateApiKey, RecordUsage};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== API Key Management Example ===\n");

    // In a real application, this would come from environment variables
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/kaccy".to_string());

    println!("Connecting to database...");
    let pool = create_pool(&database_url).await?;
    println!("✓ Connected to database\n");

    // Initialize repositories
    let api_key_repo = ApiKeyRepository::new(pool.clone());
    let usage_repo = ApiKeyUsageRepository::new(pool.clone());

    // Example user ID (in production, this would come from your user system)
    let user_id = Uuid::new_v4();

    // =========================================================================
    // 1. Creating API Keys
    // =========================================================================
    println!("1. Creating API Keys");
    println!("   -------------------");

    // Create a production API key with full permissions
    let production_key = CreateApiKey {
        user_id,
        key_hash: hash_key("prod_key_secret_123"), // In production, use proper hashing
        key_prefix: "kaccy_pr".to_string(),
        name: "Production API Key".to_string(),
        scopes: vec![
            "tokens:read".to_string(),
            "tokens:write".to_string(),
            "orders:read".to_string(),
            "orders:write".to_string(),
            "trades:read".to_string(),
        ],
        rate_limit_per_hour: Some(10000),
        expires_at: None, // No expiration for production keys
    };

    let prod_api_key = api_key_repo.create(production_key).await?;
    println!(
        "   ✓ Created production API key: {}",
        prod_api_key.key_prefix
    );
    println!("     - Scopes: {:?}", prod_api_key.scopes);
    println!("     - Rate limit: 10,000 req/hour");

    // Create a read-only API key with expiration
    let readonly_key = CreateApiKey {
        user_id,
        key_hash: hash_key("readonly_key_secret_456"),
        key_prefix: "kaccy_ro".to_string(),
        name: "Read-Only API Key".to_string(),
        scopes: vec![
            "tokens:read".to_string(),
            "orders:read".to_string(),
            "trades:read".to_string(),
        ],
        rate_limit_per_hour: Some(1000),
        expires_at: Some(Utc::now() + Duration::days(30)),
    };

    let ro_api_key = api_key_repo.create(readonly_key).await?;
    println!("   ✓ Created read-only API key: {}", ro_api_key.key_prefix);
    println!("     - Scopes: {:?}", ro_api_key.scopes);
    println!("     - Rate limit: 1,000 req/hour");
    println!(
        "     - Expires: {}",
        ro_api_key
            .expires_at
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| "Never".to_string())
    );

    // Create a temporary testing key
    let test_key = CreateApiKey {
        user_id,
        key_hash: hash_key("test_key_secret_789"),
        key_prefix: "kaccy_ts".to_string(),
        name: "Testing API Key".to_string(),
        scopes: vec!["tokens:read".to_string()],
        rate_limit_per_hour: Some(100),
        expires_at: Some(Utc::now() + Duration::hours(24)),
    };

    let test_api_key = api_key_repo.create(test_key).await?;
    println!("   ✓ Created testing API key: {}", test_api_key.key_prefix);
    println!("     - Limited to 24 hours\n");

    // =========================================================================
    // 2. Validating and Using API Keys
    // =========================================================================
    println!("2. Validating API Keys");
    println!("   -------------------");

    // Check if a key is valid
    let is_valid = api_key_repo.is_valid(&prod_api_key.key_hash).await?;
    println!("   ✓ Production key is valid: {}", is_valid);

    // Verify key has required scope
    let has_write_scope = api_key_repo
        .has_scope(prod_api_key.id, "tokens:write")
        .await?;
    println!(
        "   ✓ Production key has tokens:write scope: {}",
        has_write_scope
    );

    let ro_has_write = api_key_repo
        .has_scope(ro_api_key.id, "tokens:write")
        .await?;
    println!(
        "   ✓ Read-only key has tokens:write scope: {}",
        ro_has_write
    );

    // Update last used timestamp (called on each API request)
    api_key_repo.update_last_used(prod_api_key.id).await?;
    println!("   ✓ Updated last used timestamp\n");

    // =========================================================================
    // 3. Tracking API Usage
    // =========================================================================
    println!("3. Tracking API Usage");
    println!("   ------------------");

    // Simulate API requests
    let requests = vec![
        ("GET", "/api/v1/tokens", 200, 45),
        ("GET", "/api/v1/tokens/BTC", 200, 32),
        ("POST", "/api/v1/orders", 201, 156),
        ("GET", "/api/v1/orders", 200, 78),
        ("GET", "/api/v1/trades", 200, 92),
        ("DELETE", "/api/v1/orders/123", 404, 23),
        ("GET", "/api/v1/tokens", 200, 41),
        ("POST", "/api/v1/orders", 400, 18), // Bad request
        ("GET", "/api/v1/tokens", 200, 38),
        ("GET", "/api/v1/trades", 200, 87),
    ];

    for (method, endpoint, status, response_time) in requests {
        let usage = RecordUsage {
            api_key_id: prod_api_key.id,
            endpoint: endpoint.to_string(),
            method: method.to_string(),
            status_code: status,
            response_time_ms: response_time,
        };
        usage_repo.record(usage).await?;
    }

    println!("   ✓ Recorded 10 API requests\n");

    // =========================================================================
    // 4. Analyzing Usage Statistics
    // =========================================================================
    println!("4. Usage Statistics");
    println!("   ----------------");

    // Get overall statistics
    let stats = usage_repo.get_stats(prod_api_key.id, None).await?;
    println!("   Overall Statistics:");
    println!("   - Total requests: {}", stats.total_requests);
    println!("   - Successful: {}", stats.successful_requests);
    println!("   - Failed: {}", stats.failed_requests);
    println!(
        "   - Avg response time: {:.2}ms",
        stats.avg_response_time_ms.unwrap_or(0.0)
    );
    println!(
        "   - Min response time: {}ms",
        stats.min_response_time_ms.unwrap_or(0)
    );
    println!(
        "   - Max response time: {}ms\n",
        stats.max_response_time_ms.unwrap_or(0)
    );

    // Get per-endpoint statistics
    let endpoint_stats = usage_repo
        .get_endpoint_stats(prod_api_key.id, Some(5))
        .await?;
    println!("   Top Endpoints:");
    for (i, stat) in endpoint_stats.iter().enumerate() {
        println!(
            "   {}. {} {} - {} requests, {:.1}% success, {:.2}ms avg",
            i + 1,
            stat.method,
            stat.endpoint,
            stat.request_count,
            stat.success_rate.unwrap_or(0.0),
            stat.avg_response_time_ms.unwrap_or(0.0)
        );
    }
    println!();

    // Get current hour's request count (for rate limiting)
    let current_hour_count = usage_repo.get_current_hour_count(prod_api_key.id).await?;
    println!("   Rate Limiting:");
    println!("   - Requests this hour: {}", current_hour_count);
    println!("   - Rate limit: 10,000/hour");
    println!(
        "   - Remaining: {}\n",
        10000_i64.saturating_sub(current_hour_count)
    );

    // Get error rate
    let error_rate = usage_repo.get_error_rate(prod_api_key.id, 24).await?;
    println!("   Error Rate (24h): {:.2}%\n", error_rate);

    // =========================================================================
    // 5. Managing API Keys
    // =========================================================================
    println!("5. Managing API Keys");
    println!("   -----------------");

    // Get all active keys for user
    let active_keys = api_key_repo.get_active_keys(user_id).await?;
    println!("   Active keys: {}", active_keys.len());
    for key in &active_keys {
        println!("   - {} ({})", key.name, key.key_prefix);
    }
    println!();

    // Update key name
    api_key_repo
        .update_name(test_api_key.id, "Testing Key (Updated)")
        .await?;
    println!("   ✓ Updated key name");

    // Update scopes
    api_key_repo
        .update_scopes(
            ro_api_key.id,
            vec!["tokens:read".to_string(), "trades:read".to_string()],
        )
        .await?;
    println!("   ✓ Updated scopes for read-only key");

    // Update rate limit
    api_key_repo
        .update_rate_limit(test_api_key.id, Some(50))
        .await?;
    println!("   ✓ Updated rate limit for testing key\n");

    // =========================================================================
    // 6. Key Lifecycle Management
    // =========================================================================
    println!("6. Key Lifecycle");
    println!("   --------------");

    // Revoke a key
    api_key_repo.revoke(test_api_key.id).await?;
    println!("   ✓ Revoked testing key");

    // Verify it's no longer valid
    let is_revoked_valid = api_key_repo.is_valid(&test_api_key.key_hash).await?;
    println!("   ✓ Revoked key is valid: {}", is_revoked_valid);

    // Clean up expired keys
    let cleaned = api_key_repo.cleanup_expired().await?;
    println!("   ✓ Cleaned up {} expired keys\n", cleaned);

    // =========================================================================
    // 7. Advanced Analytics
    // =========================================================================
    println!("7. Advanced Analytics");
    println!("   -------------------");

    // Get hourly usage trend
    let hourly_usage = usage_repo.get_hourly_usage(prod_api_key.id, 24).await?;
    println!("   Hourly usage (last 24h): {} entries", hourly_usage.len());

    // Get response time trend
    let response_trend = usage_repo
        .get_response_time_trend(prod_api_key.id, 24)
        .await?;
    println!(
        "   Response time trend: {} data points",
        response_trend.len()
    );

    // Get recent usage records
    let recent = usage_repo.get_recent(prod_api_key.id, Some(5)).await?;
    println!("\n   Recent requests:");
    for record in recent {
        println!(
            "   - {} {} -> {} ({}ms)",
            record.method, record.endpoint, record.status_code, record.response_time_ms
        );
    }

    println!("\n=== Example Complete ===");
    println!(
        "\nNote: In production, you would:\n\
         - Use proper password hashing (Argon2, bcrypt) for key_hash\n\
         - Implement proper rate limiting middleware\n\
         - Set up monitoring and alerts based on usage stats\n\
         - Implement key rotation policies\n\
         - Use the cleanup functions in scheduled jobs"
    );

    Ok(())
}

/// Mock key hashing function (use proper hashing in production!)
fn hash_key(key: &str) -> String {
    // In production, use argon2::hash_encoded or similar
    // This is just for demonstration
    format!("hashed_{}", key)
}
