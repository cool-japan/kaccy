//! Simple CRUD Application Example
//!
//! This example demonstrates basic CRUD operations using kaccy-db:
//! - Creating a database connection pool
//! - Creating, reading, updating, and deleting users
//! - Basic error handling
//! - Transaction usage
//!
//! Run with: cargo run --example simple_crud

use kaccy_db::{error::Result, pool::create_pool};
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Simple CRUD Example ===\n");

    // 1. Create connection pool
    println!("1. Creating database connection pool...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/kaccy_dev".to_string());

    let pool = create_pool(&database_url).await?;
    println!("✓ Pool created successfully\n");

    // 2. Create a new user
    println!("2. Creating a new user...");
    let user_id = create_user_example(&pool).await?;
    println!("✓ User created with ID: {}\n", user_id);

    // 3. Read the user
    println!("3. Reading user by ID...");
    let user = read_user_example(&pool, user_id).await?;
    println!("✓ User found: {}\n", user);

    // 4. Update the user
    println!("4. Updating user profile...");
    update_user_example(&pool, user_id).await?;
    println!("✓ User updated\n");

    // 5. Verify update
    println!("5. Verifying update...");
    let updated_user = read_user_example(&pool, user_id).await?;
    println!("✓ Updated user: {}\n", updated_user);

    // 6. Delete the user
    println!("6. Deleting user...");
    delete_user_example(&pool, user_id).await?;
    println!("✓ User deleted\n");

    // 7. Verify deletion
    println!("7. Verifying deletion...");
    match read_user_example(&pool, user_id).await {
        Ok(_) => println!("⚠ User still exists (unexpected)"),
        Err(_) => println!("✓ User not found (expected)"),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}

async fn create_user_example(pool: &sqlx::PgPool) -> Result<i64> {
    use chrono::Utc;
    use uuid::Uuid;

    // Generate unique email
    let email = format!("user_{}@example.com", Uuid::new_v4());
    let now = Utc::now();

    let row = sqlx::query(
        r#"
        INSERT INTO users (email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(&email)
    .bind("example_user")
    .bind("hashed_password_here") // In production, use proper password hashing!
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}

async fn read_user_example(pool: &sqlx::PgPool, user_id: i64) -> Result<String> {
    let row = sqlx::query(
        r#"
        SELECT id, email, username, reputation_score
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let id: i64 = row.get("id");
    let email: String = row.get("email");
    let username: String = row.get("username");
    let reputation_score: Option<i32> = row.get("reputation_score");

    Ok(format!(
        "User(id={}, email={}, username={}, reputation={})",
        id,
        email,
        username,
        reputation_score.unwrap_or(0)
    ))
}

async fn update_user_example(pool: &sqlx::PgPool, user_id: i64) -> Result<()> {
    use chrono::Utc;

    sqlx::query(
        r#"
        UPDATE users
        SET username = $1, updated_at = $2
        WHERE id = $3
        "#,
    )
    .bind("updated_username")
    .bind(Utc::now())
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn delete_user_example(pool: &sqlx::PgPool, user_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM users WHERE id = $1
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

// Advanced example: Using transactions
#[allow(dead_code)]
async fn transaction_example(pool: &sqlx::PgPool) -> Result<()> {
    use chrono::Utc;

    println!("\n=== Transaction Example ===\n");

    // Start transaction
    let mut tx = pool.begin().await?;
    let now = Utc::now();

    // Create user
    let row = sqlx::query(
        r#"
        INSERT INTO users (email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind("transaction_user@example.com")
    .bind("tx_user")
    .bind("hashed_password")
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    let user_id: i64 = row.get("id");
    println!("Created user in transaction: {}", user_id);

    // Create token (requires user to exist)
    let row = sqlx::query(
        r#"
        INSERT INTO tokens (symbol, name, total_supply, issuer_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind("TXC")
    .bind("Transaction Coin")
    .bind(rust_decimal::Decimal::new(1_000_000, 0))
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    let token_id: i64 = row.get("id");
    println!("Created token in transaction: {}", token_id);

    // Commit transaction (both operations succeed together)
    tx.commit().await?;
    println!("✓ Transaction committed");

    // Clean up
    sqlx::query("DELETE FROM tokens WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

// Bulk operations example
#[allow(dead_code)]
async fn bulk_operations_example(pool: &sqlx::PgPool) -> Result<()> {
    use chrono::Utc;

    println!("\n=== Bulk Operations Example ===\n");

    // Create multiple users at once
    let emails: Vec<String> = (0..5)
        .map(|i| format!("bulk_user_{}@example.com", i))
        .collect();

    let mut tx = pool.begin().await?;
    let now = Utc::now();

    let mut user_ids: Vec<i64> = Vec::new();
    for email in &emails {
        let row = sqlx::query(
            r#"
            INSERT INTO users (email, username, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(email)
        .bind("bulk_user")
        .bind("hashed_password")
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        user_ids.push(row.get("id"));
    }

    tx.commit().await?;
    println!("✓ Created {} users in bulk", user_ids.len());

    // Clean up
    for user_id in user_ids {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

// Error handling example
#[allow(dead_code)]
async fn error_handling_example(pool: &sqlx::PgPool) -> Result<()> {
    use chrono::Utc;

    println!("\n=== Error Handling Example ===\n");

    let now = Utc::now();

    // Try to create user with duplicate email
    let result = sqlx::query(
        r#"
        INSERT INTO users (email, username, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind("duplicate@example.com")
    .bind("user1")
    .bind("hash")
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(_) => println!("User created"),
        Err(e) => {
            // Handle specific database errors
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    println!("⚠ Email already exists (unique constraint violation)");
                } else if db_err.is_foreign_key_violation() {
                    println!("⚠ Foreign key constraint violation");
                } else {
                    println!("⚠ Database error: {}", db_err);
                }
            } else {
                println!("⚠ Other error: {}", e);
            }
        }
    }

    Ok(())
}
