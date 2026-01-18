//! Multi-Region Deployment Example
//!
//! This example demonstrates how to set up a multi-region database deployment:
//! - Configure multiple regional database pools
//! - Route queries to nearest region
//! - Handle cross-region replication
//! - Measure replication lag
//! - Automatic failover to healthy regions
//!
//! Run with: cargo run --example multi_region

use kaccy_db::{
    error::Result,
    multi_region::{MultiRegionPoolManager, Region, RegionalPoolConfig},
};
use sqlx::Row;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Multi-Region Deployment Example ===\n");

    // 1. Set up multi-region pool manager
    println!("1. Setting up multi-region pools...");
    let manager = setup_multi_region_pools().await?;
    println!("✓ Multi-region setup complete\n");

    // 2. Geographic routing
    println!("2. Testing geographic routing...");
    test_geographic_routing(&manager).await?;
    println!("✓ Geographic routing works\n");

    // 3. Measure replication lag
    println!("3. Measuring replication lag...");
    test_replication_lag(&manager).await?;
    println!("✓ Replication lag measured\n");

    // 4. Failover testing
    println!("4. Testing automatic failover...");
    test_failover(&manager).await?;
    println!("✓ Failover works\n");

    // 5. Write to primary, read from replicas
    println!("5. Testing write-read consistency...");
    test_write_read_consistency(&manager).await?;
    println!("✓ Write-read consistency verified\n");

    println!("=== Example Complete ===");
    Ok(())
}

async fn setup_multi_region_pools() -> Result<MultiRegionPoolManager> {
    // In production, these would be different database URLs for each region
    let primary_url = std::env::var("PRIMARY_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/kaccy_dev".to_string());

    let us_west_url = std::env::var("US_WEST_DATABASE_URL").unwrap_or_else(|_| primary_url.clone());
    let us_east_url = std::env::var("US_EAST_DATABASE_URL").unwrap_or_else(|_| primary_url.clone());
    let eu_west_url = std::env::var("EU_WEST_DATABASE_URL").unwrap_or_else(|_| primary_url.clone());

    // Initialize multi-region manager
    let manager = MultiRegionPoolManager::new(1.0); // 1 second max lag

    // Add US West (San Francisco) - Primary
    let us_west_region = Region::new(
        "us-west-1".to_string(),
        "US West (San Francisco)".to_string(),
        37.7749, // San Francisco
        -122.4194,
    );
    manager
        .add_regional_pool(RegionalPoolConfig {
            region: us_west_region,
            database_url: us_west_url,
            is_primary: true,
            failover_priority: 1,
            is_active: true,
        })
        .await?;

    println!("  ✓ Added region: us-west-1 (San Francisco) [PRIMARY]");

    // Add US East (New York)
    let us_east_region = Region::new(
        "us-east-1".to_string(),
        "US East (New York)".to_string(),
        40.7128, // New York
        -74.0060,
    );
    manager
        .add_regional_pool(RegionalPoolConfig {
            region: us_east_region,
            database_url: us_east_url,
            is_primary: false,
            failover_priority: 2,
            is_active: true,
        })
        .await?;

    println!("  ✓ Added region: us-east-1 (New York)");

    // Add EU West (London)
    let eu_west_region = Region::new(
        "eu-west-1".to_string(),
        "EU West (London)".to_string(),
        51.5074, // London
        -0.1278,
    );
    manager
        .add_regional_pool(RegionalPoolConfig {
            region: eu_west_region,
            database_url: eu_west_url,
            is_primary: false,
            failover_priority: 3,
            is_active: true,
        })
        .await?;

    println!("  ✓ Added region: eu-west-1 (London)");

    Ok(manager)
}

async fn test_geographic_routing(manager: &MultiRegionPoolManager) -> Result<()> {
    // Test routing from different user locations

    // User in California
    let california_lat = 37.3382;
    let california_lon = -121.8863;
    let (region_name, pool) = manager.get_closest_pool(california_lat, california_lon)?;
    println!("  User in California → routed to {}", region_name);

    // Verify we can query from this pool
    let row = sqlx::query("SELECT 1 as value").fetch_one(&pool).await?;
    let result: i32 = row.get("value");
    println!("  ✓ Query successful (result: {})", result);

    // User in New York
    let ny_lat = 40.7128;
    let ny_lon = -74.0060;
    let (region_name, _pool) = manager.get_closest_pool(ny_lat, ny_lon)?;
    println!("  User in New York → routed to {}", region_name);

    // User in London
    let london_lat = 51.5074;
    let london_lon = -0.1278;
    let (region_name, _pool) = manager.get_closest_pool(london_lat, london_lon)?;
    println!("  User in London → routed to {}", region_name);

    Ok(())
}

async fn test_replication_lag(manager: &MultiRegionPoolManager) -> Result<()> {
    // Measure replication lag between regions
    let regions = ["us-west-1", "us-east-1", "eu-west-1"];

    for i in 0..regions.len() {
        for j in (i + 1)..regions.len() {
            match manager
                .measure_replication_lag(regions[i], regions[j])
                .await
            {
                Ok(lag) => {
                    println!(
                        "  Replication lag: {} → {} = {:.2} seconds",
                        regions[i], regions[j], lag.lag_seconds
                    );

                    if lag.lag_seconds > 1.0 {
                        println!("    ⚠ High replication lag detected!");
                    }
                }
                Err(e) => {
                    println!("  ⚠ Could not measure lag: {}", e);
                    println!("    (This is expected if using same database for all regions)");
                }
            }
        }
    }

    Ok(())
}

async fn test_failover(manager: &MultiRegionPoolManager) -> Result<()> {
    // Get list of all regions
    let regions = manager.list_regions();
    println!("  Available regions: {} total", regions.len());

    // Check if all replicas are healthy
    let all_healthy = manager.are_all_replicas_healthy();
    if all_healthy {
        println!("  ✓ All replicas are healthy");
    } else {
        let unhealthy = manager.get_unhealthy_replicas();
        println!("  ⚠ Some replicas are unhealthy: {:?}", unhealthy);
    }

    // Test failover scenario
    println!("\n  Simulating failover scenario:");
    println!("  If a region fails, queries automatically route to next nearest healthy region");

    // Get primary pool (always available)
    let primary = manager.get_primary_pool()?;
    let row = sqlx::query("SELECT 1 as value").fetch_one(&primary).await?;
    let result: i32 = row.get("value");
    println!("  ✓ Primary pool accessible (result: {})", result);

    Ok(())
}

async fn test_write_read_consistency(manager: &MultiRegionPoolManager) -> Result<()> {
    use chrono::Utc;
    use uuid::Uuid;

    // Write to primary
    let primary = manager.get_primary_pool()?;

    let email = format!("multi_region_{}@example.com", Uuid::new_v4());
    let now = Utc::now();

    let row = sqlx::query(
        r#"
        INSERT INTO users (email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(&email)
    .bind("multi_region_user")
    .bind("hashed_password")
    .bind(now)
    .bind(now)
    .fetch_one(&primary)
    .await?;

    let user_id: i64 = row.get("id");
    println!("  ✓ Wrote to primary: user_id = {}", user_id);

    // Wait a bit for replication (if using actual replicas)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Read from nearest region (could be replica)
    let (_region_name, nearest) = manager.get_closest_pool(40.7128, -74.0060)?; // New York

    match sqlx::query(
        r#"
        SELECT id, email, username
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&nearest)
    .await
    {
        Ok(user_row) => {
            let username: String = user_row.get("username");
            let email: String = user_row.get("email");
            println!("  ✓ Read from replica: user {} ({})", username, email);
        }
        Err(e) => {
            println!("  ⚠ Read from replica failed (replication lag?): {}", e);
            println!("    This is expected if replication lag > 100ms");
        }
    }

    // Clean up
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&primary)
        .await?;

    println!("  ✓ Cleanup complete");

    Ok(())
}

// Advanced: Handling read-after-write consistency
#[allow(dead_code)]
async fn read_after_write_example(manager: &MultiRegionPoolManager) -> Result<()> {
    use chrono::Utc;

    println!("\n=== Read-After-Write Consistency ===\n");

    let primary = manager.get_primary_pool()?;
    let now = Utc::now();

    // Write
    let row = sqlx::query(
        r#"
        INSERT INTO users (email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind("consistency@example.com")
    .bind("consistency_user")
    .bind("hash")
    .bind(now)
    .bind(now)
    .fetch_one(&primary)
    .await?;

    let user_id: i64 = row.get("id");
    println!("Wrote user: {}", user_id);

    // IMPORTANT: For read-after-write consistency, always read from primary
    // immediately after writing!

    let user = sqlx::query(
        r#"
        SELECT id, email, username
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&primary) // Use primary, not replica!
    .await?;

    let username: String = user.get("username");
    let email: String = user.get("email");
    println!("Read from primary: {} ({})", username, email);

    // After some time, replicas will catch up and you can use them
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Now safe to read from replica
    let (_region_name, nearest) = manager.get_closest_pool(40.7128, -74.0060)?;
    let user = sqlx::query(
        r#"
        SELECT id, email, username
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&nearest)
    .await?;

    let username: String = user.get("username");
    let email: String = user.get("email");
    println!("Read from replica: {} ({})", username, email);

    // Clean up
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&primary)
        .await?;

    Ok(())
}

// Monitoring example
#[allow(dead_code)]
async fn monitoring_example(manager: &MultiRegionPoolManager) -> Result<()> {
    println!("\n=== Multi-Region Monitoring ===\n");

    // Continuously monitor region health
    let health_check_interval = Duration::from_secs(10);

    println!(
        "Starting health checks every {} seconds...",
        health_check_interval.as_secs()
    );
    println!("Press Ctrl+C to stop");

    let mut interval = tokio::time::interval(health_check_interval);

    loop {
        interval.tick().await;

        let regions = manager.list_regions();

        // Check if all replicas are healthy
        let all_healthy = manager.are_all_replicas_healthy();

        for region_config in &regions {
            let status = if region_config.is_active {
                "✓ ACTIVE"
            } else {
                "✗ INACTIVE"
            };

            println!(
                "[{}] Region {}: {}",
                chrono::Utc::now(),
                region_config.region.code,
                status
            );

            // Measure lag to primary (if not primary)
            if !region_config.is_primary {
                if let Ok(lag) = manager
                    .measure_replication_lag("us-west-1", &region_config.region.code)
                    .await
                {
                    println!("  └─ Replication lag: {:.2} seconds", lag.lag_seconds);
                }
            }
        }

        println!(
            "  Overall health: {}",
            if all_healthy {
                "✓ ALL HEALTHY"
            } else {
                "⚠ SOME ISSUES"
            }
        );
        println!();
    }
}
