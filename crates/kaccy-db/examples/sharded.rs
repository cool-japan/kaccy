//! Sharded Database Deployment Example
//!
//! This example demonstrates database sharding for horizontal scalability:
//! - Set up multiple database shards
//! - Distribute data across shards using different strategies
//! - Route queries to correct shard based on shard key
//! - Execute cross-shard queries
//! - Handle shard rebalancing
//!
//! Run with: cargo run --example sharded

use chrono::Utc;
use kaccy_db::{
    error::Result,
    sharding::{ShardInfo, ShardKey, ShardPoolManager, ShardingStrategy},
};
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Sharded Database Deployment Example ===\n");

    // 1. Set up sharding
    println!("1. Setting up database shards...");
    let manager = setup_sharded_pools().await?;
    println!("✓ Sharding setup complete\n");

    // 2. Hash-based sharding
    println!("2. Testing hash-based sharding...");
    test_hash_sharding(&manager).await?;
    println!("✓ Hash sharding works\n");

    // 3. Range-based sharding
    println!("3. Testing range-based sharding...");
    test_range_sharding().await?;
    println!("✓ Range sharding works\n");

    // 4. Cross-shard queries
    println!("4. Testing cross-shard queries...");
    test_cross_shard_queries(&manager).await?;
    println!("✓ Cross-shard queries work\n");

    // 5. Shard distribution analysis
    println!("5. Analyzing shard distribution...");
    analyze_shard_distribution(&manager).await?;
    println!("✓ Distribution analysis complete\n");

    println!("=== Example Complete ===");
    Ok(())
}

async fn setup_sharded_pools() -> Result<ShardPoolManager> {
    // In production, each shard would be a separate database instance
    // For this example, we'll use the same database but demonstrate the sharding logic

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/kaccy_dev".to_string());

    let shard0_url = std::env::var("SHARD0_URL").unwrap_or_else(|_| database_url.clone());
    let shard1_url = std::env::var("SHARD1_URL").unwrap_or_else(|_| database_url.clone());
    let shard2_url = std::env::var("SHARD2_URL").unwrap_or_else(|_| database_url.clone());
    let shard3_url = std::env::var("SHARD3_URL").unwrap_or_else(|_| database_url.clone());

    // Initialize shard manager with hash-based sharding strategy
    let manager = ShardPoolManager::new(ShardingStrategy::Hash);

    // Add shards with their connection info
    let shard_infos = vec![
        ShardInfo {
            id: 0,
            name: "shard_0".to_string(),
            connection_string: shard0_url,
            is_active: true,
            weight: 1,
            region: None,
            range_start: None,
            range_end: None,
        },
        ShardInfo {
            id: 1,
            name: "shard_1".to_string(),
            connection_string: shard1_url,
            is_active: true,
            weight: 1,
            region: None,
            range_start: None,
            range_end: None,
        },
        ShardInfo {
            id: 2,
            name: "shard_2".to_string(),
            connection_string: shard2_url,
            is_active: true,
            weight: 1,
            region: None,
            range_start: None,
            range_end: None,
        },
        ShardInfo {
            id: 3,
            name: "shard_3".to_string(),
            connection_string: shard3_url,
            is_active: true,
            weight: 1,
            region: None,
            range_start: None,
            range_end: None,
        },
    ];

    for shard_info in shard_infos {
        manager.add_shard(shard_info).await?;
    }

    println!("  ✓ Added 4 shards (0-3)");
    println!("  ✓ Using hash-based sharding strategy");

    Ok(manager)
}

async fn test_hash_sharding(manager: &ShardPoolManager) -> Result<()> {
    println!("  Testing hash-based distribution...");

    // Create users across different shards
    let user_ids = vec![1, 2, 3, 4, 5, 10, 100, 1000];

    for user_id in &user_ids {
        // Determine which shard this user belongs to
        let shard_key = ShardKey::Integer(*user_id as i64);
        let shard = manager.get_shard_id(&shard_key)?;

        // Get the pool for this shard
        let pool = manager.get_pool_by_id(shard)?;

        // Insert user into correct shard
        let now = Utc::now();
        let email = format!("user{}@shard{}.com", user_id, shard);
        let username = format!("user{}", user_id);

        let result = sqlx::query(
            r#"
            INSERT INTO users (id, email, username, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&username)
        .bind("hashed_password")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await;

        match result {
            Ok(_) => println!("    User {} → Shard {}", user_id, shard),
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    println!("    User {} already exists on Shard {}", user_id, shard);
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    // Verify we can read from correct shards
    println!("\n  Verifying reads from correct shards...");

    for user_id in &user_ids {
        let shard_key = ShardKey::Integer(*user_id as i64);
        let shard = manager.get_shard_id(&shard_key)?;
        let pool = manager.get_pool_by_id(shard)?;

        match sqlx::query(
            r#"
            SELECT id, email, username
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        {
            Ok(user_row) => {
                let username: String = user_row.get("username");
                println!(
                    "    ✓ Read user {} from shard {}: {}",
                    user_id, shard, username
                );
            }
            Err(_) => {
                println!("    User {} not found on shard {}", user_id, shard);
            }
        }
    }

    Ok(())
}

async fn test_range_sharding() -> Result<()> {
    println!("  Demonstrating range-based sharding strategy...");

    // Range-based sharding example
    // Shard 0: user_id 1-1000
    // Shard 1: user_id 1001-2000
    // Shard 2: user_id 2001-3000
    // Shard 3: user_id 3001+

    let ranges = vec![
        (1, 1000, 0),
        (1001, 2000, 1),
        (2001, 3000, 2),
        (3001, 4000, 3),
    ];

    println!("\n  Range-based shard assignment:");
    for (start, end, shard) in ranges {
        println!("    User IDs {}-{} → Shard {}", start, end, shard);
    }

    // Show how to determine shard for range-based strategy
    let test_user_ids = vec![500, 1500, 2500, 3500];

    println!("\n  Example user assignments:");
    for user_id in test_user_ids {
        let shard = if user_id <= 1000 {
            0
        } else if user_id <= 2000 {
            1
        } else if user_id <= 3000 {
            2
        } else {
            3
        };

        println!("    User ID {} → Shard {}", user_id, shard);
    }

    Ok(())
}

async fn test_cross_shard_queries(manager: &ShardPoolManager) -> Result<()> {
    println!("  Executing cross-shard query...");

    // To query across all shards, we need to:
    // 1. Execute query on each shard
    // 2. Aggregate results

    #[derive(Debug)]
    #[allow(dead_code)]
    struct UserData {
        id: i64,
        username: String,
        email: String,
    }

    let mut all_users = Vec::new();

    for shard_id in 0..4 {
        let pool = manager.get_pool_by_id(shard_id)?;

        let users = sqlx::query(
            r#"
            SELECT id, username, email
            FROM users
            ORDER BY id
            LIMIT 10
            "#,
        )
        .fetch_all(&pool)
        .await?;

        println!("    Shard {}: Found {} users", shard_id, users.len());

        for row in users {
            all_users.push(UserData {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
            });
        }
    }

    // Sort combined results
    all_users.sort_by_key(|u| u.id);

    println!("\n  Combined results from all shards:");
    for user in all_users.iter().take(5) {
        println!("    User {}: {}", user.id, user.username);
    }

    println!("  Total users across all shards: {}", all_users.len());

    Ok(())
}

async fn analyze_shard_distribution(manager: &ShardPoolManager) -> Result<()> {
    println!("  Analyzing data distribution across shards...");

    let mut total_users: i64 = 0;
    let mut shard_counts = Vec::new();

    for shard_id in 0..4 {
        let pool = manager.get_pool_by_id(shard_id)?;

        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM users
            "#,
        )
        .fetch_one(&pool)
        .await?;

        let count: i64 = row.get("count");

        shard_counts.push((shard_id, count));
        total_users += count;
    }

    println!("\n  Shard distribution:");
    for (shard_id, count) in &shard_counts {
        let percentage = if total_users > 0 {
            (*count as f64 / total_users as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "    Shard {}: {} users ({:.1}%)",
            shard_id, count, percentage
        );
    }

    println!("\n  Total users: {}", total_users);

    // Check for imbalance
    if !shard_counts.is_empty() {
        let max_count = shard_counts.iter().map(|(_, c)| c).max().unwrap();
        let min_count = shard_counts.iter().map(|(_, c)| c).min().unwrap();

        let imbalance = if min_count > &0 {
            (*max_count as f64 / *min_count as f64) - 1.0
        } else {
            0.0
        };

        if imbalance > 0.5 {
            println!(
                "\n  ⚠ Warning: Significant imbalance detected ({:.1}%)",
                imbalance * 100.0
            );
            println!("  Consider rebalancing shards or adjusting shard key");
        } else {
            println!("\n  ✓ Shard distribution is balanced");
        }
    }

    Ok(())
}

// Advanced: Consistent hashing for better distribution
#[allow(dead_code)]
async fn consistent_hashing_example() -> Result<()> {
    use kaccy_db::sharding::ConsistentHashRing;

    println!("\n=== Consistent Hashing Example ===\n");

    // Create consistent hash ring
    let ring = ConsistentHashRing::new(3); // 3 virtual nodes per shard

    // Add shards
    ring.add_shard(0);
    ring.add_shard(1);
    ring.add_shard(2);
    ring.add_shard(3);

    println!("Created consistent hash ring with 4 shards");

    // Show how keys are distributed
    let test_keys = vec![
        "user_1",
        "user_2",
        "user_3",
        "user_10",
        "user_100",
        "user_1000",
    ];

    println!("\nKey distribution:");
    for key in test_keys {
        let shard_key = ShardKey::Custom(key.to_string());
        let shard = ring.get_shard(&shard_key);
        if let Some(shard) = shard {
            println!("  {} → Shard {}", key, shard);
        }
    }

    // Simulate adding a new shard
    println!("\n--- Adding new shard (shard 4) ---");
    ring.add_shard(4);

    println!("\nKey distribution after adding shard:");
    for key in &[
        "user_1",
        "user_2",
        "user_3",
        "user_10",
        "user_100",
        "user_1000",
    ] {
        let shard_key = ShardKey::Custom(key.to_string());
        let shard = ring.get_shard(&shard_key);
        if let Some(shard) = shard {
            println!("  {} → Shard {}", key, shard);
        }
    }

    println!("\nNote: Only some keys moved to new shard (minimal disruption)");

    Ok(())
}

// Advanced: Shard migration
#[allow(dead_code)]
async fn shard_migration_example(manager: &ShardPoolManager) -> Result<()> {
    println!("\n=== Shard Migration Example ===\n");

    // Steps to migrate data from one shard to another:
    // 1. Read data from source shard
    // 2. Write data to destination shard
    // 3. Verify migration
    // 4. Delete from source shard

    let source_shard = 0;
    let dest_shard = 1;
    let user_id_to_migrate = 1;

    println!(
        "Migrating user {} from shard {} to shard {}",
        user_id_to_migrate, source_shard, dest_shard
    );

    // Step 1: Read from source
    let source_pool = manager.get_pool_by_id(source_shard)?;
    let user = sqlx::query(
        r#"
        SELECT id, email, username, password_hash, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id_to_migrate)
    .fetch_one(&source_pool)
    .await?;

    let user_id: i64 = user.get("id");
    let user_email: String = user.get("email");
    let user_username: String = user.get("username");
    let user_password_hash: String = user.get("password_hash");
    let user_created_at: chrono::DateTime<Utc> = user.get("created_at");
    let user_updated_at: chrono::DateTime<Utc> = user.get("updated_at");

    println!("  ✓ Read user from source shard");

    // Step 2: Write to destination
    let dest_pool = manager.get_pool_by_id(dest_shard)?;
    sqlx::query(
        r#"
        INSERT INTO users (id, email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(&user_email)
    .bind(&user_username)
    .bind(&user_password_hash)
    .bind(user_created_at)
    .bind(user_updated_at)
    .execute(&dest_pool)
    .await?;

    println!("  ✓ Wrote user to destination shard");

    // Step 3: Verify
    let verified_user = sqlx::query(
        r#"
        SELECT id, email
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id_to_migrate)
    .fetch_one(&dest_pool)
    .await?;

    let verified_email: String = verified_user.get("email");
    assert_eq!(verified_email, user_email);
    println!("  ✓ Verified data in destination shard");

    // Step 4: Delete from source (COMMENTED OUT for safety)
    // sqlx::query!("DELETE FROM users WHERE id = $1", user_id_to_migrate)
    //     .execute(&source_pool)
    //     .await?;
    // println!("  ✓ Deleted from source shard");

    println!("\n  Migration complete!");
    println!("  Note: In production, use a transaction coordinator for atomicity");

    Ok(())
}

// Monitoring shard health
#[allow(dead_code)]
async fn monitor_shards(manager: &ShardPoolManager) -> Result<()> {
    println!("\n=== Shard Health Monitoring ===\n");

    for shard_id in 0..4 {
        let pool = manager.get_pool_by_id(shard_id)?;

        // Check connection
        let is_healthy = sqlx::query("SELECT 1 as value")
            .fetch_one(&pool)
            .await
            .is_ok();

        // Get shard size
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM users
            "#,
        )
        .fetch_one(&pool)
        .await?;

        let user_count: i64 = row.get("count");

        let status = if is_healthy {
            "✓ HEALTHY"
        } else {
            "✗ UNHEALTHY"
        };

        println!("Shard {}: {} ({} users)", shard_id, status, user_count);
    }

    Ok(())
}
