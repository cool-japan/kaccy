/// Comprehensive example demonstrating the Background Job Queue System
///
/// This example shows how to use the BackgroundJobRepository for:
/// - Creating and scheduling jobs
/// - Polling and processing jobs
/// - Handling job failures and retries
/// - Job lifecycle management
/// - Monitoring and statistics
///
/// Run with: cargo run --example background_jobs
use chrono::{Duration, Utc};
use kaccy_db::{BackgroundJobRepository, CreateBackgroundJob, JobStatus};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // NOTE: This example requires a running PostgreSQL database
    // Set DATABASE_URL environment variable or use default
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/kaccy".to_string());

    println!("🚀 Background Job Queue System Example\n");
    println!("Connecting to database: {}\n", database_url);

    // Create database connection pool
    let pool = PgPool::connect(&database_url).await?;

    // Create repository
    let repo = BackgroundJobRepository::new(pool.clone());

    println!("=== 1. Creating Background Jobs ===\n");

    // Example 1: Send email job
    let email_job = CreateBackgroundJob {
        job_type: "send_email".to_string(),
        payload: json!({
            "to": "user@example.com",
            "subject": "Welcome to Kaccy!",
            "template": "welcome",
            "user_id": Uuid::new_v4()
        }),
        priority: Some(10), // High priority
        max_retries: Some(3),
        retry_backoff_ms: Some(1000),
        scheduled_at: None, // Run immediately
        expires_at: Some(Utc::now() + Duration::hours(24)),
        created_by: Some(Uuid::new_v4()),
        correlation_id: Some("user-signup-flow-123".to_string()),
    };

    let job1 = repo.create(email_job).await?;
    println!("✅ Created email job: {}", job1.id);
    println!("   Status: {}", job1.status);
    println!("   Priority: {}", job1.priority);
    println!("   Scheduled: {}\n", job1.scheduled_at);

    // Example 2: Process blockchain confirmation (delayed job)
    let blockchain_job = CreateBackgroundJob {
        job_type: "process_blockchain_confirmation".to_string(),
        payload: json!({
            "txid": "abc123...",
            "order_id": Uuid::new_v4(),
            "confirmations": 0
        }),
        priority: Some(5),     // Medium priority
        max_retries: Some(10), // More retries for blockchain operations
        retry_backoff_ms: Some(5000),
        scheduled_at: Some(Utc::now() + Duration::minutes(5)), // Check in 5 minutes
        expires_at: Some(Utc::now() + Duration::days(7)),
        created_by: None,
        correlation_id: Some("order-confirmation-456".to_string()),
    };

    let job2 = repo.create(blockchain_job).await?;
    println!("✅ Created blockchain confirmation job: {}", job2.id);
    println!("   Scheduled for: {}\n", job2.scheduled_at);

    // Example 3: Batch job creation
    let batch_jobs = vec![
        CreateBackgroundJob {
            job_type: "generate_report".to_string(),
            payload: json!({"report_type": "daily_trades", "date": "2026-01-08"}),
            priority: None,
            max_retries: None,
            retry_backoff_ms: None,
            scheduled_at: None,
            expires_at: None,
            created_by: Some(Uuid::new_v4()),
            correlation_id: Some("daily-reports".to_string()),
        },
        CreateBackgroundJob {
            job_type: "cleanup_expired_data".to_string(),
            payload: json!({"table": "temp_tokens", "days": 30}),
            priority: Some(1), // Low priority
            max_retries: None,
            retry_backoff_ms: None,
            scheduled_at: None,
            expires_at: None,
            created_by: None,
            correlation_id: Some("maintenance".to_string()),
        },
    ];

    let created_batch = repo.batch_create(batch_jobs).await?;
    println!("✅ Batch created {} jobs\n", created_batch.len());

    println!("=== 2. Polling for Jobs ===\n");

    // Poll for next available job (any type)
    if let Some(next_job) = repo.poll_next_job(None).await? {
        println!("📥 Polled job: {}", next_job.id);
        println!("   Type: {}", next_job.job_type);
        println!(
            "   Status: {} (automatically set to 'running')",
            next_job.status
        );
        println!("   Started at: {:?}\n", next_job.started_at);

        // Simulate job processing
        println!("⚙️  Processing job...\n");
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Mark as completed with result
        let completed = repo
            .mark_completed(
                next_job.id,
                Some(json!({
                    "message": "Email sent successfully",
                    "sent_at": Utc::now(),
                })),
            )
            .await?;
        println!("✅ Job completed: {}", completed.id);
        println!("   Result: {:?}\n", completed.result);
    }

    // Poll for specific job types (email and report jobs only)
    if let Some(specific_job) = repo
        .poll_next_job(Some(vec![
            "send_email".to_string(),
            "generate_report".to_string(),
        ]))
        .await?
    {
        println!("📥 Polled specific job type: {}", specific_job.job_type);

        // Simulate failure
        let failed = repo
            .mark_failed(
                specific_job.id,
                "SMTP server temporarily unavailable".to_string(),
            )
            .await?;
        println!("❌ Job failed: {}", failed.id);
        println!("   Error: {}", failed.error_message.unwrap_or_default());
        println!(
            "   Retry count: {}/{}\n",
            failed.retry_count, failed.max_retries
        );

        // Retry the failed job
        let retried = repo.retry_job(failed.id).await?;
        println!("🔄 Job retried: {}", retried.id);
        println!("   Status: {}", retried.status);
        println!(
            "   Next scheduled: {} (with backoff)\n",
            retried.scheduled_at
        );
    }

    println!("=== 3. Querying Jobs ===\n");

    // Get pending jobs
    let pending_jobs = repo.get_by_status(JobStatus::Pending, 1, 10).await?;
    println!("📋 Pending jobs: {}", pending_jobs.len());

    // Get jobs by type
    let email_jobs = repo.get_by_type("send_email", 1, 10).await?;
    println!("📧 Email jobs: {}", email_jobs.len());

    // Get jobs by correlation ID (distributed tracing)
    let correlated_jobs = repo.get_by_correlation_id("daily-reports").await?;
    println!(
        "🔗 Jobs with correlation ID 'daily-reports': {}\n",
        correlated_jobs.len()
    );

    println!("=== 4. Job Statistics ===\n");

    // Get statistics by job type
    let stats = repo.get_type_statistics().await?;
    println!("📊 Job Type Statistics:");
    for stat in stats {
        println!("   {} →", stat.job_type);
        println!("     Pending: {}", stat.pending_count);
        println!("     Running: {}", stat.running_count);
        println!("     Completed: {}", stat.completed_count);
        println!("     Failed: {}", stat.failed_count);
        if let Some(avg_duration) = stat.average_duration_seconds {
            println!("     Avg Duration: {:.2}s", avg_duration);
        }
    }
    println!();

    // Get queue depth
    let queue_depth = repo.get_queue_depth().await?;
    println!(
        "📈 Current queue depth: {} jobs ready to process\n",
        queue_depth
    );

    // Count by status
    let completed_count = repo.count_by_status(JobStatus::Completed).await?;
    let failed_count = repo.count_by_status(JobStatus::Failed).await?;
    println!("✅ Completed jobs: {}", completed_count);
    println!("❌ Failed jobs: {}\n", failed_count);

    println!("=== 5. Job Maintenance ===\n");

    // Find stuck jobs (running for more than 30 minutes)
    let stuck_jobs = repo.get_stuck_jobs(30).await?;
    println!("⚠️  Stuck jobs (running > 30 min): {}", stuck_jobs.len());
    if !stuck_jobs.is_empty() {
        println!("   Resetting stuck jobs...");
        let reset_count = repo.reset_stuck_jobs(30).await?;
        println!("   ✅ Reset {} stuck jobs to pending\n", reset_count);
    }

    // Cleanup old completed jobs (older than 30 days)
    println!("🧹 Cleaning up old completed jobs...");
    let cleaned_count = repo.cleanup_old_jobs(30).await?;
    println!("   ✅ Deleted {} old completed jobs\n", cleaned_count);

    // Cleanup expired jobs
    println!("🧹 Cleaning up expired jobs...");
    let expired_count = repo.cleanup_expired_jobs().await?;
    println!("   ✅ Deleted {} expired jobs\n", expired_count);

    println!("=== 6. Job Lifecycle Example ===\n");

    // Create a job
    let lifecycle_job = CreateBackgroundJob {
        job_type: "data_export".to_string(),
        payload: json!({"format": "csv", "records": 1000}),
        priority: Some(5),
        max_retries: Some(3),
        retry_backoff_ms: Some(2000),
        scheduled_at: None,
        expires_at: Some(Utc::now() + Duration::hours(1)),
        created_by: Some(Uuid::new_v4()),
        correlation_id: Some("export-flow".to_string()),
    };

    let job = repo.create(lifecycle_job).await?;
    println!("1. Created: {} (status: {})", job.id, job.status);

    // Find by ID
    let found = repo.find_by_id(job.id).await?;
    println!("2. Found: {:?}", found.is_some());

    // Cancel the job
    let cancelled = repo.cancel_job(job.id).await?;
    println!(
        "3. Cancelled: {} (status: {})\n",
        cancelled.id, cancelled.status
    );

    println!("=== Worker Pattern Example ===\n");
    println!("💡 Typical worker implementation:");
    println!("   1. Poll for next job: poll_next_job(Some(vec![\"my_job_type\"]))");
    println!("   2. Process the job payload");
    println!("   3. On success: mark_completed(job_id, result)");
    println!("   4. On failure: mark_failed(job_id, error)");
    println!("   5. Jobs automatically retry with exponential backoff");
    println!("   6. Use correlation_id for distributed tracing\n");

    println!("✅ Example completed successfully!");
    println!("\n📚 Key Features:");
    println!("   • Priority-based job execution");
    println!("   • Automatic retry with exponential backoff");
    println!("   • Job scheduling (delayed execution)");
    println!("   • Job expiration");
    println!("   • Distributed tracing with correlation IDs");
    println!("   • Atomic job polling (FOR UPDATE SKIP LOCKED)");
    println!("   • Comprehensive statistics and monitoring");
    println!("   • Automatic cleanup and maintenance");

    Ok(())
}
