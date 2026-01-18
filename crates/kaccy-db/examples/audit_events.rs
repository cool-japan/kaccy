//! Audit Events Example
//!
//! This example demonstrates how to use the AuditEventRepository for
//! event-driven architecture integration, compliance tracking, and distributed tracing.
//!
//! Run with: cargo run --example audit_events

use chrono::{Duration, Utc};
use kaccy_db::{create_pool, AuditEventRepository, CreateAuditEvent};
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Audit Events Example ===\n");

    // Create database connection pool
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/kaccy".to_string());
    let pool = create_pool(&database_url).await?;
    println!("✓ Connected to database\n");

    // Create repository
    let repo = AuditEventRepository::new(pool.clone());

    // Example 1: Create audit events for different scenarios
    println!("--- Example 1: Creating Audit Events ---");

    let user_id = Uuid::new_v4();
    let correlation_id = format!("trace-{}", Uuid::new_v4());

    // Event 1: User registration
    let event1 = CreateAuditEvent {
        id: Uuid::new_v4(),
        event_type: "UserRegistered".to_string(),
        event_data: json!({
            "username": "alice",
            "email": "alice@example.com",
            "registration_method": "email",
            "ip_address": "192.168.1.100"
        }),
        source: "kaccy-api".to_string(),
        correlation_id: Some(correlation_id.clone()),
        user_id: Some(user_id),
    };

    let created1 = repo.create(event1).await?;
    println!("✓ Created event: {} ({})", created1.event_type, created1.id);

    // Event 2: Email verification
    let event2 = CreateAuditEvent {
        id: Uuid::new_v4(),
        event_type: "EmailVerified".to_string(),
        event_data: json!({
            "email": "alice@example.com",
            "verification_token": "abc123"
        }),
        source: "kaccy-api".to_string(),
        correlation_id: Some(correlation_id.clone()),
        user_id: Some(user_id),
    };

    let created2 = repo.create(event2).await?;
    println!("✓ Created event: {} ({})", created2.event_type, created2.id);

    // Event 3: Token created
    let event3 = CreateAuditEvent {
        id: Uuid::new_v4(),
        event_type: "TokenCreated".to_string(),
        event_data: json!({
            "symbol": "ALICE",
            "total_supply": 1000000,
            "decimals": 8
        }),
        source: "kaccy-core".to_string(),
        correlation_id: Some(correlation_id.clone()),
        user_id: Some(user_id),
    };

    let created3 = repo.create(event3).await?;
    println!("✓ Created event: {} ({})", created3.event_type, created3.id);

    // Event 4: Order placed (different user, different correlation)
    let user_id2 = Uuid::new_v4();
    let correlation_id2 = format!("trace-{}", Uuid::new_v4());

    let event4 = CreateAuditEvent {
        id: Uuid::new_v4(),
        event_type: "OrderPlaced".to_string(),
        event_data: json!({
            "order_id": Uuid::new_v4().to_string(),
            "side": "buy",
            "amount": 100,
            "price": 0.001
        }),
        source: "kaccy-core".to_string(),
        correlation_id: Some(correlation_id2.clone()),
        user_id: Some(user_id2),
    };

    let created4 = repo.create(event4).await?;
    println!(
        "✓ Created event: {} ({})\n",
        created4.event_type, created4.id
    );

    // Example 2: Query events by user
    println!("--- Example 2: Query Events by User ---");
    let user_events = repo.get_by_user(user_id, 10, 0).await?;
    println!("Found {} events for user {}", user_events.len(), user_id);
    for event in &user_events {
        println!(
            "  - {} at {} (source: {})",
            event.event_type, event.timestamp, event.source
        );
    }
    println!();

    // Example 3: Query events by event type
    println!("--- Example 3: Query Events by Type ---");
    let type_events = repo.get_by_event_type("UserRegistered", 10, 0).await?;
    println!("Found {} UserRegistered events", type_events.len());
    for event in &type_events {
        println!(
            "  - {} (user: {:?})",
            event.timestamp,
            event
                .user_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
    }
    println!();

    // Example 4: Query events by source system
    println!("--- Example 4: Query Events by Source ---");
    let source_events = repo.get_by_source("kaccy-core", 10, 0).await?;
    println!("Found {} events from kaccy-core", source_events.len());
    for event in &source_events {
        println!("  - {} at {}", event.event_type, event.timestamp);
    }
    println!();

    // Example 5: Distributed tracing - get all events in a correlation chain
    println!("--- Example 5: Distributed Tracing ---");
    let correlated_events = repo.get_by_correlation(&correlation_id).await?;
    println!(
        "Found {} events in correlation chain '{}'",
        correlated_events.len(),
        correlation_id
    );
    for event in &correlated_events {
        println!(
            "  - {} at {} (source: {})",
            event.event_type, event.timestamp, event.source
        );
    }

    // Get correlation trace with timing
    if let Some(trace) = repo.get_correlation_trace(&correlation_id).await? {
        println!("\nCorrelation Trace Analysis:");
        println!("  Total events: {}", trace.event_count);
        println!("  Event types: {:?}", trace.event_types);
        println!("  Sources: {:?}", trace.sources);
        println!("  Duration: {:.2}ms", trace.duration_ms);
        println!("  Time span: {} to {}", trace.start_time, trace.end_time);
    }
    println!();

    // Example 6: Event type statistics
    println!("--- Example 6: Event Type Statistics ---");
    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    let stats = repo.get_event_type_stats(yesterday, now).await?;
    println!("Event statistics for the last 24 hours:");
    for stat in stats {
        println!("  {} events:", stat.event_type);
        println!("    Count: {}", stat.event_count);
        println!("    Unique users: {}", stat.unique_users);
        println!("    Unique correlations: {}", stat.unique_correlations);
        println!(
            "    First: {}, Last: {}",
            stat.first_occurrence, stat.last_occurrence
        );
    }
    println!();

    // Example 7: Daily event summary
    println!("--- Example 7: Daily Event Summary ---");
    let today = Utc::now().date_naive();
    let week_ago = today - chrono::Days::new(7);
    let daily_summaries = repo.get_daily_summary(week_ago, today).await?;
    println!("Daily summaries for the last week:");
    for summary in daily_summaries {
        println!("  Date: {}", summary.event_date);
        println!("    Total events: {}", summary.total_events);
        println!("    Unique event types: {}", summary.unique_event_types);
        println!("    Unique sources: {}", summary.unique_sources);
        println!("    Unique users: {}", summary.unique_users);
    }
    println!();

    // Example 8: User activity summary
    println!("--- Example 8: User Activity Summary ---");
    let user_activity = repo.get_user_activity(yesterday, now, 10).await?;
    println!("Top user activity in the last 24 hours:");
    for activity in user_activity {
        println!("  User {}:", activity.user_id);
        println!("    Total events: {}", activity.total_events);
        println!("    Unique event types: {}", activity.unique_event_types);
        println!(
            "    Activity period: {} to {}",
            activity.first_event, activity.last_event
        );
    }
    println!();

    // Example 9: Search by JSONB field
    println!("--- Example 9: Search by JSONB Field ---");
    let search_results = repo
        .search_by_json_field("registration_method", "email", 10, 0)
        .await?;
    println!(
        "Found {} events with registration_method=email",
        search_results.len()
    );
    for event in &search_results {
        println!(
            "  - {} at {} (data: {})",
            event.event_type, event.timestamp, event.event_data
        );
    }
    println!();

    // Example 10: Count events
    println!("--- Example 10: Event Counts ---");
    let total_count = repo.count_all().await?;
    println!("Total events in database: {}", total_count);

    let user_count = repo.count_by_user(user_id).await?;
    println!("Events for user {}: {}", user_id, user_count);

    let type_count = repo.count_by_event_type("UserRegistered").await?;
    println!("UserRegistered events: {}", type_count);
    println!();

    // Example 11: Query by time range
    println!("--- Example 11: Query by Time Range ---");
    let end_time = Utc::now();
    let start_time = end_time - Duration::hours(1);
    let recent_events = repo.get_by_time_range(start_time, end_time, 10, 0).await?;
    println!("Found {} events in the last hour", recent_events.len());
    for event in &recent_events {
        println!(
            "  - {} at {} ({})",
            event.event_type, event.timestamp, event.source
        );
    }
    println!();

    // Example 12: GDPR compliance - cleanup old events
    println!("--- Example 12: GDPR Compliance - Data Cleanup ---");
    let retention_cutoff = Utc::now() - Duration::days(365); // 1 year retention
    println!(
        "Note: Would delete events older than {} for GDPR compliance",
        retention_cutoff
    );
    println!("(Skipping actual deletion in this example)");

    // Uncomment to actually delete old events:
    // let deleted_count = repo.delete_older_than(&pool, retention_cutoff).await?;
    // println!("Deleted {} old events", deleted_count);
    println!();

    // Example 13: Event-driven workflow tracking
    println!("--- Example 13: Event-Driven Workflow Tracking ---");
    println!("Creating a complete workflow with correlation tracking...");

    let workflow_correlation = format!("workflow-{}", Uuid::new_v4());
    let workflow_user = Uuid::new_v4();

    // Simulate a complete token issuance workflow
    let workflow_events = vec![
        (
            "RequestReceived",
            json!({"endpoint": "/api/tokens", "method": "POST"}),
        ),
        ("ValidationStarted", json!({"validator": "TokenValidator"})),
        (
            "ValidationPassed",
            json!({"checks": ["symbol_unique", "supply_valid"]}),
        ),
        (
            "TokenCreated",
            json!({"symbol": "WORKFLOW", "supply": 1000000}),
        ),
        (
            "BalanceUpdated",
            json!({"user_id": workflow_user.to_string(), "amount": 1000000}),
        ),
        (
            "NotificationSent",
            json!({"type": "email", "recipient": "user@example.com"}),
        ),
        (
            "WorkflowCompleted",
            json!({"duration_ms": 250, "status": "success"}),
        ),
    ];

    for (event_type, event_data) in workflow_events {
        let event = CreateAuditEvent {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            event_data,
            source: "kaccy-api".to_string(),
            correlation_id: Some(workflow_correlation.clone()),
            user_id: Some(workflow_user),
        };
        repo.create(event).await?;
        println!("  ✓ {}", event_type);
    }

    // Analyze the workflow
    if let Some(trace) = repo.get_correlation_trace(&workflow_correlation).await? {
        println!("\nWorkflow Analysis:");
        println!("  Steps completed: {}", trace.event_count);
        println!("  Total duration: {:.2}ms", trace.duration_ms);
        println!("  Event flow: {:?}", trace.event_types);
    }
    println!();

    println!("=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("1. Audit events provide comprehensive tracking for compliance");
    println!("2. Correlation IDs enable distributed tracing across services");
    println!("3. JSONB storage allows flexible event data structures");
    println!("4. Rich querying supports analytics and debugging");
    println!("5. GDPR-compliant cleanup maintains data retention policies");

    Ok(())
}
