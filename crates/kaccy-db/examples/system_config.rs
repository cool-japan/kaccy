/// Comprehensive example demonstrating the System Configuration Management
///
/// This example shows how to use the SystemConfigRepository for:
/// - Creating and managing dynamic configurations
/// - Feature flags and A/B testing
/// - Configuration versioning and history
/// - Environment-specific settings
/// - Batch updates and rollbacks
/// - Type-safe configuration access
///
/// Run with: cargo run --example system_config
use kaccy_db::{CreateSystemConfig, SystemConfigRepository, UpdateSystemConfig, ValueType};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // NOTE: This example requires a running PostgreSQL database
    // Set DATABASE_URL environment variable or use default
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/kaccy".to_string());

    println!("🔧 System Configuration Management Example\n");
    println!("Connecting to database: {}\n", database_url);

    // Create database connection pool
    let pool = PgPool::connect(&database_url).await?;

    // Create repository
    let repo = SystemConfigRepository::new(pool.clone());

    let admin_user_id = Uuid::new_v4();

    println!("=== 1. Creating Configurations ===\n");

    // Example 1: API Rate Limit (Number)
    let api_limit = CreateSystemConfig {
        config_key: "max_api_requests_per_hour".to_string(),
        config_value: "1000".to_string(),
        value_type: ValueType::Number,
        description: Some("Maximum API requests per hour per user".to_string()),
        category: Some("api_limits".to_string()),
        environment: Some("production".to_string()),
        validation_rules: Some(json!({"min": 100, "max": 10000})),
        is_sensitive: Some(false),
        changed_by: Some(admin_user_id),
        changed_reason: Some("Initial configuration".to_string()),
        effective_from: None,
        effective_until: None,
    };

    let config1 = repo.create(api_limit).await?;
    println!("✅ Created API limit config: {}", config1.config_key);
    println!(
        "   Value: {} (version {})",
        config1.config_value, config1.version
    );
    println!("   Category: {}\n", config1.category.unwrap_or_default());

    // Example 2: Feature Flag (Boolean)
    let feature_flag = CreateSystemConfig {
        config_key: "feature_advanced_trading".to_string(),
        config_value: "true".to_string(),
        value_type: ValueType::Boolean,
        description: Some("Enable advanced trading features".to_string()),
        category: Some("feature_flags".to_string()),
        environment: Some("production".to_string()),
        validation_rules: None,
        is_sensitive: Some(false),
        changed_by: Some(admin_user_id),
        changed_reason: Some("Launch advanced trading".to_string()),
        effective_from: None,
        effective_until: None,
    };

    let config2 = repo.create(feature_flag).await?;
    println!("✅ Created feature flag: {}", config2.config_key);
    println!("   Enabled: {}\n", config2.config_value);

    // Example 3: Complex Configuration (JSON)
    let ui_config = CreateSystemConfig {
        config_key: "ui_theme_settings".to_string(),
        config_value: json!({
            "primary_color": "#3B82F6",
            "secondary_color": "#10B981",
            "dark_mode_enabled": true,
            "font_family": "Inter"
        })
        .to_string(),
        value_type: ValueType::Json,
        description: Some("UI theme configuration".to_string()),
        category: Some("ui_settings".to_string()),
        environment: Some("all".to_string()),
        validation_rules: None,
        is_sensitive: Some(false),
        changed_by: Some(admin_user_id),
        changed_reason: Some("Brand update".to_string()),
        effective_from: None,
        effective_until: None,
    };

    let config3 = repo.create(ui_config).await?;
    println!("✅ Created UI theme config: {}", config3.config_key);
    println!(
        "   Environment: {} (applies to all environments)\n",
        config3.environment
    );

    // Example 4: Sensitive Configuration (API Key)
    let sensitive_config = CreateSystemConfig {
        config_key: "external_api_key".to_string(),
        config_value: "sk_live_abc123xyz789".to_string(),
        value_type: ValueType::String,
        description: Some("External service API key".to_string()),
        category: Some("api_keys".to_string()),
        environment: Some("production".to_string()),
        validation_rules: None,
        is_sensitive: Some(true),
        changed_by: Some(admin_user_id),
        changed_reason: Some("Service integration".to_string()),
        effective_from: None,
        effective_until: None,
    };

    let config4 = repo.create(sensitive_config).await?;
    println!("✅ Created sensitive config: {}", config4.config_key);
    println!("   Sensitive: {}\n", config4.is_sensitive);

    println!("=== 2. Retrieving Configurations ===\n");

    // Get by key
    if let Some(config) = repo
        .get_by_key("max_api_requests_per_hour", Some("production"))
        .await?
    {
        println!("📥 Retrieved config: {}", config.config_key);
        println!("   Value: {}", config.config_value);
        println!(
            "   Description: {}\n",
            config.description.unwrap_or_default()
        );
    }

    // Get all active configs for production
    let all_configs = repo.get_all(Some("production")).await?;
    println!("📋 All production configs: {}", all_configs.len());
    for config in &all_configs {
        println!("   • {} = {}", config.config_key, config.config_value);
    }
    println!();

    // Get by category
    let api_configs = repo
        .get_by_category("api_limits", Some("production"))
        .await?;
    println!("🏷️  API limit configs: {}", api_configs.len());
    for config in &api_configs {
        println!("   • {} = {}", config.config_key, config.config_value);
    }
    println!();

    println!("=== 3. Type-Safe Configuration Access ===\n");

    // Get as specific types
    if let Some(limit) = repo
        .get_number("max_api_requests_per_hour", Some("production"))
        .await?
    {
        println!("🔢 API limit (as number): {}", limit);
    }

    if let Some(enabled) = repo
        .get_bool("feature_advanced_trading", Some("production"))
        .await?
    {
        println!("✅ Feature enabled (as bool): {}", enabled);
    }

    if let Some(theme) = repo.get_json("ui_theme_settings", None).await? {
        println!("🎨 UI theme (as JSON):");
        println!("   {}\n", serde_json::to_string_pretty(&theme)?);
    }

    // Feature flag checking
    let is_enabled = repo
        .is_feature_enabled("feature_advanced_trading", Some("production"))
        .await?;
    println!(
        "🚀 Advanced trading feature: {}\n",
        if is_enabled { "ENABLED" } else { "DISABLED" }
    );

    println!("=== 4. Updating Configurations ===\n");

    // Update API limit
    let updated = repo
        .update(
            "max_api_requests_per_hour",
            Some("production"),
            UpdateSystemConfig {
                config_value: "2000".to_string(),
                description: Some("Increased for premium users".to_string()),
                changed_by: Some(admin_user_id),
                changed_reason: Some("Handle increased load".to_string()),
                effective_from: None,
                effective_until: None,
            },
        )
        .await?;

    println!("🔄 Updated config: {}", updated.config_key);
    println!(
        "   Old value: {}",
        updated.previous_value.unwrap_or_default()
    );
    println!("   New value: {}", updated.config_value);
    println!(
        "   Version: {} → {}\n",
        updated.version - 1,
        updated.version
    );

    println!("=== 5. Configuration History ===\n");

    // Get change history
    let history = repo.get_history("max_api_requests_per_hour", 10).await?;
    println!(
        "📚 Configuration history for 'max_api_requests_per_hour': {} changes",
        history.len()
    );
    for (idx, change) in history.iter().enumerate() {
        println!("\n   Change #{}:", idx + 1);
        println!("   Type: {}", change.change_type);
        println!(
            "   Old value: {}",
            change.old_value.as_ref().unwrap_or(&"N/A".to_string())
        );
        println!("   New value: {}", change.new_value);
        println!(
            "   Reason: {}",
            change.changed_reason.as_ref().unwrap_or(&"N/A".to_string())
        );
        println!("   Time: {}", change.created_at);
    }
    println!();

    // Get all versions
    let versions = repo
        .get_versions("max_api_requests_per_hour", Some("production"))
        .await?;
    println!("📦 All versions: {}", versions.len());
    for version in &versions {
        println!(
            "   v{}: {} (active: {})",
            version.version, version.config_value, version.is_active
        );
    }
    println!();

    println!("=== 6. Configuration Rollback ===\n");

    // Rollback to previous version
    let rolled_back = repo
        .rollback(
            "max_api_requests_per_hour",
            Some("production"),
            Some(admin_user_id),
            Some("Performance issues with new limit"),
        )
        .await?;

    println!("⏮️  Rolled back config: {}", rolled_back.config_key);
    println!("   Current value: {}", rolled_back.config_value);
    println!("   Version: {}\n", rolled_back.version);

    println!("=== 7. Batch Operations ===\n");

    // Batch update multiple configs
    let batch_updates = vec![
        (
            "feature_advanced_trading".to_string(),
            "false".to_string(),
            Some(admin_user_id),
            Some("Temporary disable".to_string()),
        ),
        (
            "max_api_requests_per_hour".to_string(),
            "1500".to_string(),
            Some(admin_user_id),
            Some("Balanced limit".to_string()),
        ),
    ];

    let updated_count = repo.batch_update(batch_updates).await?;
    println!("📦 Batch updated {} configurations\n", updated_count);

    println!("=== 8. Configuration Statistics ===\n");

    // Get category summary
    let summary = repo.get_category_summary().await?;
    println!("📊 Configuration by category:");
    for cat in &summary {
        println!(
            "   {} → Total: {}, Active: {}",
            cat.category, cat.total_configs, cat.active_configs
        );
    }
    println!();

    // Count active configs
    let active_count = repo.count_active(Some("production")).await?;
    println!("✅ Total active production configs: {}\n", active_count);

    println!("=== 9. Configuration Deletion ===\n");

    // Delete configuration (soft delete)
    repo.delete(
        "external_api_key",
        Some("production"),
        Some(admin_user_id),
        Some("API key rotated"),
    )
    .await?;
    println!("🗑️  Deleted config: external_api_key");

    // Verify deletion
    if repo
        .get_by_key("external_api_key", Some("production"))
        .await?
        .is_none()
    {
        println!("   ✅ Config no longer active\n");
    }

    println!("=== 10. Environment-Specific Configs ===\n");

    // Create staging-specific config
    let staging_config = CreateSystemConfig {
        config_key: "debug_mode".to_string(),
        config_value: "true".to_string(),
        value_type: ValueType::Boolean,
        description: Some("Enable debug logging".to_string()),
        category: Some("debugging".to_string()),
        environment: Some("staging".to_string()),
        validation_rules: None,
        is_sensitive: Some(false),
        changed_by: Some(admin_user_id),
        changed_reason: Some("Staging environment setup".to_string()),
        effective_from: None,
        effective_until: None,
    };

    let staging = repo.create(staging_config).await?;
    println!("✅ Created staging config: {}", staging.config_key);
    println!("   Environment: {}", staging.environment);

    // Show environment filtering
    println!(
        "\n   Production configs: {}",
        repo.count_active(Some("production")).await?
    );
    println!(
        "   Staging configs: {}",
        repo.count_active(Some("staging")).await?
    );
    println!();

    println!("✅ Example completed successfully!");
    println!("\n📚 Key Features Demonstrated:");
    println!("   • Dynamic configuration management");
    println!("   • Configuration versioning and history");
    println!("   • Feature flags and A/B testing");
    println!("   • Type-safe configuration access");
    println!("   • Environment-specific settings");
    println!("   • Batch operations");
    println!("   • Rollback capabilities");
    println!("   • Audit trail and change tracking");
    println!("   • Category-based organization");
    println!("   • Sensitive data handling");

    Ok(())
}
