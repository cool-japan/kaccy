//! Example: Database Schema Validation
//!
//! This example demonstrates how to use the schema validator to ensure
//! database integrity, detect orphaned records, and identify potential issues.
//!
//! # Features Demonstrated
//!
//! 1. Comprehensive schema validation
//! 2. Foreign key constraint analysis
//! 3. Orphaned record detection
//! 4. Missing index identification
//! 5. Check constraint validation
//! 6. Actionable fix recommendations
//! 7. CI/CD integration examples
//!
//! # Run this example
//!
//! ```bash
//! cargo run --example schema_validation
//! ```

use kaccy_db::{create_pool, validate_schema, ValidationSeverity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Database Schema Validation Example ===\n");

    // Create a connection pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/kaccy_dev".to_string());

    println!("Connecting to database...");
    let pool = create_pool(&database_url).await?;
    println!("✓ Connected to database\n");

    // Example 1: Comprehensive schema validation
    println!("--- Example 1: Comprehensive Schema Validation ---");
    let report = validate_schema(&pool).await?;

    println!("Schema Validation Report:");
    println!("  Validated at: {}", report.validated_at);
    println!("  Total issues: {}", report.issues.len());
    println!("  - Critical: {}", report.critical_count);
    println!("  - Errors: {}", report.error_count);
    println!("  - Warnings: {}", report.warning_count);
    println!("  - Info: {}", report.info_count);

    // Example 2: Foreign key analysis
    println!("\n--- Example 2: Foreign Key Analysis ---");
    println!(
        "Found {} foreign key constraints",
        report.foreign_keys.len()
    );

    let fks_without_indexes: Vec<_> = report
        .foreign_keys
        .iter()
        .filter(|fk| !fk.has_index)
        .collect();

    if fks_without_indexes.is_empty() {
        println!("✓ All foreign keys have indexes");
    } else {
        println!(
            "⚠️  {} foreign keys missing indexes:",
            fks_without_indexes.len()
        );
        for fk in fks_without_indexes.iter().take(5) {
            println!(
                "  - {}.{} -> {}.{}",
                fk.table_name, fk.column_name, fk.referenced_table, fk.referenced_column
            );
        }
    }

    // Example 3: Orphaned record detection
    println!("\n--- Example 3: Orphaned Record Detection ---");
    if report.orphaned_records.is_empty() {
        println!("✓ No orphaned records found");
    } else {
        println!(
            "⚠️  Found orphaned records in {} tables:",
            report.orphaned_records.len()
        );
        for orphaned in &report.orphaned_records {
            println!(
                "  - {}.{}: {} orphaned records (references {})",
                orphaned.table_name,
                orphaned.column_name,
                orphaned.orphaned_count,
                orphaned.referenced_table
            );
        }
    }

    // Example 4: Check constraint validation
    println!("\n--- Example 4: Check Constraint Validation ---");
    println!("Found {} check constraints", report.check_constraints.len());

    let unvalidated: Vec<_> = report
        .check_constraints
        .iter()
        .filter(|c| !c.is_validated)
        .collect();

    if unvalidated.is_empty() {
        println!("✓ All check constraints are validated");
    } else {
        println!("⚠️  {} unvalidated constraints:", unvalidated.len());
        for constraint in unvalidated.iter().take(3) {
            println!(
                "  - {}.{}",
                constraint.table_name, constraint.constraint_name
            );
        }
    }

    // Example 5: Issue breakdown by severity
    println!("\n--- Example 5: Issue Breakdown by Severity ---");

    if report.critical_count > 0 {
        println!("\n🚨 CRITICAL ISSUES:");
        for issue in report
            .issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Critical)
        {
            println!("  Category: {}", issue.category);
            println!("  Object: {}", issue.affected_object);
            println!("  Problem: {}", issue.description);
            println!("  Fix: {}", issue.recommendation);
            println!();
        }
    }

    if report.error_count > 0 {
        println!("⚠️⚠️  ERRORS:");
        for issue in report
            .issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
        {
            println!("  Category: {}", issue.category);
            println!("  Object: {}", issue.affected_object);
            println!("  Problem: {}", issue.description);
            println!("  Fix: {}", issue.recommendation);
            println!();
        }
    }

    if report.warning_count > 0 {
        println!("⚠️  WARNINGS:");
        for (i, issue) in report
            .issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
            .take(5)
            .enumerate()
        {
            println!(
                "  {}. {} - {} ({})",
                i + 1,
                issue.category,
                issue.affected_object,
                issue.description
            );
        }
        if report.warning_count > 5 {
            println!("  ... and {} more warnings", report.warning_count - 5);
        }
    }

    // Example 6: Actionable recommendations
    println!("\n--- Example 6: Actionable Fix Recommendations ---");
    println!("Generating SQL scripts to fix issues...\n");

    // Group recommendations by category
    let mut index_recommendations = Vec::new();
    let mut constraint_recommendations = Vec::new();

    for issue in &report.issues {
        if issue.category == "Missing Index" {
            index_recommendations.push(&issue.recommendation);
        } else if issue.category == "Unvalidated Constraint" {
            constraint_recommendations.push(&issue.recommendation);
        }
    }

    if !index_recommendations.is_empty() {
        println!("-- Add missing indexes");
        for rec in index_recommendations.iter().take(3) {
            println!("{}", rec);
        }
        if index_recommendations.len() > 3 {
            println!(
                "-- ... and {} more indexes\n",
                index_recommendations.len() - 3
            );
        }
    }

    if !constraint_recommendations.is_empty() {
        println!("-- Validate constraints");
        for rec in constraint_recommendations.iter().take(3) {
            println!("{}", rec);
        }
        if constraint_recommendations.len() > 3 {
            println!(
                "-- ... and {} more validations\n",
                constraint_recommendations.len() - 3
            );
        }
    }

    // Example 7: CI/CD integration
    println!("\n--- Example 7: CI/CD Integration ---");
    println!("Exit codes for automated pipelines:");

    let exit_code = if report.critical_count > 0 {
        println!("  Exit 2: Critical issues found - deployment blocked");
        2
    } else if report.error_count > 0 {
        println!("  Exit 1: Errors found - manual review required");
        1
    } else if report.warning_count > 0 {
        println!("  Exit 0: Warnings found - deployment allowed with notification");
        0
    } else {
        println!("  Exit 0: No issues - deployment approved");
        0
    };

    // Example 8: JSON export for monitoring systems
    println!("\n--- Example 8: JSON Export for Monitoring ---");
    let json = serde_json::to_string_pretty(&report)?;
    println!("Schema validation report can be exported as JSON:");
    println!("  Size: {} bytes", json.len());
    println!("  Use for: Prometheus alerts, Slack notifications, dashboards");

    // Example 9: Health scoring
    println!("\n--- Example 9: Schema Health Score ---");
    let total_checks =
        report.foreign_keys.len() + report.check_constraints.len() + report.orphaned_records.len();

    let health_score = if total_checks > 0 {
        ((total_checks - report.issues.len()) as f64 / total_checks as f64) * 100.0
    } else {
        100.0
    };

    println!("Schema Health Score: {:.1}%", health_score);

    if health_score >= 95.0 {
        println!("  Status: ✅ Excellent");
    } else if health_score >= 85.0 {
        println!("  Status: ✓ Good");
    } else if health_score >= 70.0 {
        println!("  Status: ⚠️  Fair - improvement needed");
    } else {
        println!("  Status: 🚨 Poor - immediate action required");
    }

    // Example 10: Scheduled validation workflow
    println!("\n--- Example 10: Recommended Validation Schedule ---");
    println!("Schedule schema validation:");
    println!("  - Daily: Run during maintenance window");
    println!("  - Pre-deployment: Block on critical issues");
    println!("  - Post-migration: Verify schema integrity");
    println!("  - Weekly: Full report with trend analysis");

    println!("\n=== Schema Validation Example Complete ===");
    println!("Exit code: {}", exit_code);

    Ok(())
}
