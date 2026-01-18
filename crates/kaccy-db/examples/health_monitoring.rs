//! Example: Comprehensive Health Monitoring System
//!
//! This example demonstrates how to use the health check system to monitor
//! database health across multiple subsystems including connection pools,
//! query performance, locks, tables, cache, and storage.
//!
//! # Features Demonstrated
//!
//! 1. Basic health check with default configuration
//! 2. Custom health check with configurable thresholds
//! 3. Quick health check for high-frequency probes
//! 4. Health status interpretation and alerting
//! 5. Detailed diagnostic information extraction
//! 6. Integration with monitoring systems (Prometheus, etc.)
//!
//! # Run this example
//!
//! ```bash
//! cargo run --example health_monitoring
//! ```

use kaccy_db::{
    create_pool, perform_health_check, quick_health_check, HealthCheckConfig, HealthStatus,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Database Health Monitoring Example ===\n");

    // Create a connection pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/kaccy_dev".to_string());

    println!("Connecting to database...");
    let pool = create_pool(&database_url).await?;
    println!("✓ Connected to database\n");

    // Example 1: Basic health check with default configuration
    println!("--- Example 1: Basic Health Check (Default Config) ---");
    let config = HealthCheckConfig::default();
    println!("Configuration:");
    println!(
        "  - Pool utilization warning: {:.0}%",
        config.pool_utilization_warning * 100.0
    );
    println!(
        "  - Pool utilization critical: {:.0}%",
        config.pool_utilization_critical * 100.0
    );
    println!(
        "  - Cache hit ratio warning: {:.0}%",
        config.cache_hit_ratio_warning * 100.0
    );
    println!(
        "  - Slow query threshold: {}ms",
        config.slow_query_threshold_ms
    );
    println!("  - Max blocking queries: {}", config.max_blocking_queries);
    println!(
        "  - Max lock wait time: {}s\n",
        config.max_lock_wait_seconds
    );

    let report = perform_health_check(&pool, &config).await?;
    display_health_report(&report);

    // Example 2: Custom health check with stricter thresholds
    println!("\n--- Example 2: Custom Health Check (Strict Thresholds) ---");
    let strict_config = HealthCheckConfig {
        pool_utilization_warning: 0.6,
        pool_utilization_critical: 0.85,
        cache_hit_ratio_warning: 0.98,
        cache_hit_ratio_critical: 0.95,
        slow_query_threshold_ms: 500,
        max_blocking_queries: 2,
        max_lock_wait_seconds: 5,
        ..Default::default()
    };

    let strict_report = perform_health_check(&pool, &strict_config).await?;
    display_health_report(&strict_report);

    // Example 3: Quick health check (for high-frequency probes)
    println!("\n--- Example 3: Quick Health Check ---");
    println!("Performing quick health check (only critical components)...");
    let quick_status = quick_health_check(&pool).await?;
    println!("Quick status: {:?}", quick_status);
    println!("Use this for Kubernetes liveness probes or high-frequency monitoring\n");

    // Example 4: Health status interpretation
    println!("--- Example 4: Health Status Interpretation ---");
    interpret_health_status(&report.overall_status);

    // Example 5: Category-specific health details
    println!("\n--- Example 5: Category-Specific Details ---");
    for check in &report.checks {
        if check.status != HealthStatus::Healthy {
            println!("\nCategory: {:?}", check.category);
            println!("Status: {:?}", check.status);
            println!("Message: {}", check.message);
            if let Some(details) = &check.details {
                println!(
                    "Details: {}",
                    serde_json::to_string_pretty(details).unwrap()
                );
            }
        }
    }

    // Example 6: Alerting logic based on health status
    println!("\n--- Example 6: Alerting Logic ---");
    if report.critical_issues > 0 {
        println!(
            "🚨 CRITICAL ALERT: {} critical issues detected!",
            report.critical_issues
        );
        println!("Action: Trigger PagerDuty/Slack critical alert");
    } else if report.warnings > 0 {
        println!("⚠️  WARNING: {} warnings detected", report.warnings);
        println!("Action: Send Slack notification to ops channel");
    } else {
        println!("✅ All systems healthy - no action needed");
    }

    // Example 7: Prometheus metrics integration
    println!("\n--- Example 7: Prometheus Metrics Integration ---");
    println!("# Example metrics you could expose:");
    println!(
        "database_health_status{{overall=\"{:?}\"}} 1",
        report.overall_status
    );
    println!("database_health_issues_total{{}} {}", report.total_issues);
    println!(
        "database_health_critical_issues{{}} {}",
        report.critical_issues
    );
    println!("database_health_warnings{{}} {}", report.warnings);
    println!(
        "database_health_check_duration_ms{{}} {}",
        report.generation_time_ms
    );

    for check in &report.checks {
        let status_value = match check.status {
            HealthStatus::Healthy => 0,
            HealthStatus::Degraded => 1,
            HealthStatus::Unhealthy => 2,
            HealthStatus::Critical => 3,
        };
        println!(
            "database_health_category_status{{category=\"{:?}\"}} {}",
            check.category, status_value
        );
    }

    // Example 8: Health check scheduling
    println!("\n--- Example 8: Health Check Scheduling ---");
    println!("Recommended scheduling:");
    println!("  - Quick check: Every 5-10 seconds (liveness probe)");
    println!("  - Full check: Every 30-60 seconds (monitoring)");
    println!("  - Detailed check: Every 5 minutes (deep diagnostics)");

    println!("\n=== Health Monitoring Example Complete ===");
    Ok(())
}

/// Display a formatted health report
fn display_health_report(report: &kaccy_db::SystemHealthReport) {
    println!("Health Report:");
    println!("  Overall Status: {:?}", report.overall_status);
    println!("  Total Issues: {}", report.total_issues);
    println!("  Critical Issues: {}", report.critical_issues);
    println!("  Warnings: {}", report.warnings);
    println!("  Generation Time: {}ms", report.generation_time_ms);
    println!("  Generated At: {}", report.generated_at);
    println!("\nCategory Breakdown:");

    for check in &report.checks {
        let status_symbol = match check.status {
            HealthStatus::Healthy => "✓",
            HealthStatus::Degraded => "⚠",
            HealthStatus::Unhealthy => "⚠⚠",
            HealthStatus::Critical => "🚨",
        };

        println!(
            "  {} {:?}: {:?} - {}",
            status_symbol, check.category, check.status, check.message
        );
    }
}

/// Interpret health status and suggest actions
fn interpret_health_status(status: &HealthStatus) {
    match status {
        HealthStatus::Healthy => {
            println!("✅ System Status: HEALTHY");
            println!("   All subsystems are operating normally");
            println!("   Action: Continue monitoring");
        }
        HealthStatus::Degraded => {
            println!("⚠️  System Status: DEGRADED");
            println!("   System has minor issues but is operational");
            println!("   Action: Investigate warnings, plan maintenance");
        }
        HealthStatus::Unhealthy => {
            println!("⚠️⚠️  System Status: UNHEALTHY");
            println!("   System has serious issues affecting performance");
            println!("   Action: Immediate investigation required");
        }
        HealthStatus::Critical => {
            println!("🚨 System Status: CRITICAL");
            println!("   System is in critical state and may fail soon");
            println!("   Action: URGENT - Activate incident response");
        }
    }
}
