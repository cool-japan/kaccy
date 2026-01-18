//! Export/Import Example
//!
//! This example demonstrates:
//! - Exporting user reputation data
//! - Bulk export operations
//! - JSON serialization
//! - Import/restore workflows
//! - Backup strategies
//!
//! Run with: `cargo run --example export_import`

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Export/Import Example ===\n");

    // 1. Export Data Structure
    println!("1. User Reputation Export Structure:");
    println!("   A complete export includes:");
    println!("      - User ID and current reputation score");
    println!("      - Current tier and score components");
    println!("      - Complete tier history (all tier changes)");
    println!("      - Score history snapshots");
    println!("      - All reputation events");
    println!("      - All commitments (with evidence)");
    println!("      - Holder ratings (given and received)");
    println!("      - SBT metadata (if minted)");
    println!("      - External verifications (GitHub, LinkedIn, etc.)");
    println!();

    // 2. Use Cases
    println!("2. Export/Import Use Cases:");
    println!();
    println!("   Backup and Recovery:");
    println!("      - Regular backups of reputation data");
    println!("      - Disaster recovery");
    println!("      - Database migration");
    println!();
    println!("   Data Portability:");
    println!("      - User requests their data (GDPR)");
    println!("      - Migration to another platform");
    println!("      - Reputation verification");
    println!();
    println!("   Analytics:");
    println!("      - Historical data analysis");
    println!("      - Trend identification");
    println!("      - Reporting and auditing");
    println!();

    // 3. Export Operations
    println!("3. Export Operations:");
    println!();
    println!("   Single User Export:");
    println!("      service.export_user(user_id).await?");
    println!("      → Returns UserReputationExport with all user data");
    println!();
    println!("   Bulk Export (specific users):");
    println!("      let user_ids = vec![user1, user2, user3];");
    println!("      service.export_bulk(user_ids).await?");
    println!("      → Returns BulkReputationExport with metadata");
    println!();
    println!("   Full System Export:");
    println!("      service.export_all().await?");
    println!("      → Exports ALL users in the system");
    println!("      ⚠️  WARNING: Can be large for production systems");
    println!();

    // 4. JSON Export
    println!("4. JSON Serialization:");
    println!();
    println!("   Convert to JSON:");
    println!("      let json = service.export_to_json(user_ids).await?");
    println!("      → Pretty-printed JSON string");
    println!();
    println!("   Save to file:");
    println!("      std::fs::write(\"backup.json\", json)?");
    println!();
    println!("   JSON Structure:");
    println!("   {{");
    println!("     \"version\": \"1.0\",");
    println!("     \"exported_at\": \"2026-01-18T...\",");
    println!("     \"total_users\": 100,");
    println!("     \"users\": [");
    println!("       {{");
    println!("         \"user_id\": \"...\",");
    println!("         \"reputation_score\": 750,");
    println!("         \"tier\": \"Gold\",");
    println!("         \"components\": {{ ... }},");
    println!("         \"tier_history\": [ ... ],");
    println!("         \"score_history\": [ ... ],");
    println!("         \"events\": [ ... ],");
    println!("         \"commitments\": [ ... ],");
    println!("         \"ratings\": {{ ... }}");
    println!("       }}");
    println!("     ]");
    println!("   }}");
    println!();

    // 5. Import/Restore
    println!("5. Import/Restore Operations:");
    println!();
    println!("   Load from JSON:");
    println!("      let json = std::fs::read_to_string(\"backup.json\")?");
    println!("      let export = service.import_from_json(&json)?");
    println!();
    println!("   Restore to database:");
    println!("      // Implementation would restore each user's:");
    println!("      for user_export in export.users {{");
    println!("          // 1. Create/update user reputation score");
    println!("          // 2. Restore tier history");
    println!("          // 3. Restore score snapshots");
    println!("          // 4. Restore events");
    println!("          // 5. Restore commitments");
    println!("          // 6. Restore ratings");
    println!("      }}");
    println!();

    // 6. Best Practices
    println!("6. Export/Import Best Practices:");
    println!();
    println!("   ✅ DO:");
    println!("      - Export regularly (daily or weekly)");
    println!("      - Compress large exports (gzip)");
    println!("      - Verify exports can be imported");
    println!("      - Store exports securely (encrypted)");
    println!("      - Keep multiple backup generations");
    println!("      - Document export format version");
    println!();
    println!("   ❌ DON'T:");
    println!("      - Export without compression for large datasets");
    println!("      - Store exports in plain text (security risk)");
    println!("      - Forget to test restore process");
    println!("      - Export PII without encryption");
    println!("      - Keep only one backup");
    println!();

    // 7. Backup Strategy
    println!("7. Recommended Backup Strategy:");
    println!();
    println!("   Daily Incremental:");
    println!("      - Export users modified in last 24 hours");
    println!("      - Append to daily backup file");
    println!("      - Keep last 7 days");
    println!();
    println!("   Weekly Full:");
    println!("      - Export all users");
    println!("      - Compress and encrypt");
    println!("      - Keep last 4 weeks");
    println!();
    println!("   Monthly Archive:");
    println!("      - Full system export");
    println!("      - Long-term archival storage");
    println!("      - Keep 12 months");
    println!();

    // 8. Performance Considerations
    println!("8. Performance Considerations:");
    println!();
    println!("   Large Exports:");
    println!("      - Use pagination for bulk exports");
    println!("      - Process in batches (e.g., 1000 users at a time)");
    println!("      - Run during low-traffic periods");
    println!("      - Monitor memory usage");
    println!();
    println!("   Export Size Estimates:");
    println!("      - ~10KB per user (basic data)");
    println!("      - ~50KB per user (with full history)");
    println!("      - ~500KB per user (with extensive commitments)");
    println!();
    println!("   10,000 users ≈ 100MB - 5GB depending on history");
    println!();

    // 9. GDPR Compliance
    println!("9. GDPR Data Export:");
    println!();
    println!("   User requests their data:");
    println!("      1. Export complete user data");
    println!("      2. Convert to human-readable format (JSON)");
    println!("      3. Provide within 30 days");
    println!("      4. Include all personal data");
    println!();
    println!("   Data included:");
    println!("      - Reputation scores and history");
    println!("      - Commitments and evidence");
    println!("      - Ratings given/received");
    println!("      - All events and changes");
    println!("      - Connected external accounts");
    println!();

    // 10. Migration Workflow
    println!("10. Platform Migration Workflow:");
    println!();
    println!("   From Platform A to Platform B:");
    println!();
    println!("   Step 1: Export from Platform A");
    println!("      platform_a_service.export_all().await?");
    println!();
    println!("   Step 2: Transform data (if needed)");
    println!("      // Convert format if platforms differ");
    println!();
    println!("   Step 3: Validate export");
    println!("      // Check all required fields present");
    println!();
    println!("   Step 4: Import to Platform B");
    println!("      platform_b_service.import_from_json(&json).await?");
    println!();
    println!("   Step 5: Verify migration");
    println!("      // Compare key metrics before/after");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}
