//! Commitment Lifecycle Example
//!
//! This example demonstrates:
//! - Creating commitments
//! - Commitment status transitions
//! - Milestone-based commitments
//! - Recurring commitments
//! - Template usage
//!
//! Run with: `cargo run --example commitment_lifecycle`

use kaccy_reputation::commitment::CommitmentStatus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Commitment Lifecycle Example ===\n");

    // 1. Commitment Status State Machine
    println!("1. Commitment Status Transitions:");
    let statuses = vec![
        CommitmentStatus::Pending,
        CommitmentStatus::Completed,
        CommitmentStatus::Verified,
        CommitmentStatus::Failed,
        CommitmentStatus::Expired,
    ];

    for status in &statuses {
        println!("   Status: {:?}", status);
        println!("      - Is terminal: {}", status.is_terminal());
        println!("      - Is active: {}", status.is_active());
        println!("      - Is successful: {}", status.is_successful());
        println!("      - Is failed: {}", status.is_failed());
        println!(
            "      - Possible next states: {:?}",
            status.possible_next_states()
        );
        println!();
    }

    // 2. Valid State Transitions
    println!("2. Valid State Transition Examples:");

    let transitions = vec![
        (CommitmentStatus::Pending, CommitmentStatus::Completed, true),
        (
            CommitmentStatus::Completed,
            CommitmentStatus::Verified,
            true,
        ),
        (CommitmentStatus::Completed, CommitmentStatus::Failed, true),
        (CommitmentStatus::Pending, CommitmentStatus::Verified, false),
        (CommitmentStatus::Verified, CommitmentStatus::Failed, false),
        (CommitmentStatus::Failed, CommitmentStatus::Verified, false),
    ];

    for (from, to, expected) in transitions {
        let can_transition = from.can_transition_to(to);
        let symbol = if can_transition == expected {
            "✅"
        } else {
            "❌"
        };
        println!(
            "   {} {:?} → {:?}: {} (expected: {})",
            symbol, from, to, can_transition, expected
        );
    }
    println!();

    // 3. Milestone Commitments
    println!("3. Milestone-Based Commitments:");
    println!("   Use case: Breaking large commitments into smaller milestones");
    println!();
    println!("   Example: 'Build NFT Marketplace' commitment:");
    println!("      Milestone 1: Smart contract development (30% weight)");
    println!("      Milestone 2: Frontend implementation (40% weight)");
    println!("      Milestone 3: Testing and deployment (30% weight)");
    println!();
    println!("   Benefits:");
    println!("      - Partial credit for completed milestones");
    println!("      - Clearer progress tracking");
    println!("      - Reduced risk of all-or-nothing outcomes");
    println!();

    // 4. Recurring Commitments
    println!("4. Recurring Commitments:");
    println!("   Frequency options:");

    use kaccy_reputation::recurring::RecurrenceFrequency;
    let frequencies = vec![
        RecurrenceFrequency::Weekly,
        RecurrenceFrequency::Biweekly,
        RecurrenceFrequency::Monthly,
        RecurrenceFrequency::Quarterly,
    ];

    for freq in frequencies {
        println!(
            "      - {}: Auto-generates next commitment on schedule",
            freq
        );
    }

    println!();
    println!("   Example: Monthly community update commitment");
    println!("      - Frequency: Monthly");
    println!("      - Auto-creates new commitment each month");
    println!("      - Maintains consistent engagement");
    println!();

    // 5. Commitment Templates
    println!("5. Commitment Templates:");
    println!("   Pre-defined template categories:");

    use kaccy_reputation::templates::TemplateCategory;
    let categories = vec![
        TemplateCategory::Content,
        TemplateCategory::Development,
        TemplateCategory::Community,
        TemplateCategory::Education,
        TemplateCategory::Events,
        TemplateCategory::Support,
    ];

    for category in categories {
        println!("      - {}", category);
    }

    println!();
    println!("   Template benefits:");
    println!("      - Standardized commitment types");
    println!("      - Pre-defined verification criteria");
    println!("      - Faster commitment creation");
    println!("      - Consistent quality standards");
    println!();

    // 6. Commitment Workflow
    println!("6. Typical Commitment Workflow:");
    println!("   Step 1: Create commitment (Status: Pending)");
    println!("      - Define title, description, deadline");
    println!("      - Set evidence requirements");
    println!();
    println!("   Step 2: Work on commitment");
    println!("      - Complete the committed work");
    println!("      - Gather evidence (URLs, descriptions)");
    println!();
    println!("   Step 3: Submit evidence (Status: Completed)");
    println!("      - Upload evidence URL");
    println!("      - Add evidence description");
    println!();
    println!("   Step 4: Admin verification");
    println!("      - Admin reviews evidence");
    println!("      - Decision: Verified or Failed");
    println!();
    println!("   Step 5: Reputation update");
    println!("      - Verified: +positive reputation delta");
    println!("      - Failed: +negative reputation delta");
    println!("      - Status becomes terminal");
    println!();

    // 7. Best Practices
    println!("7. Commitment Best Practices:");
    println!("   ✅ DO:");
    println!("      - Set realistic deadlines (24 hours min, 12 months max)");
    println!("      - Provide clear, specific descriptions");
    println!("      - Use milestones for large commitments");
    println!("      - Submit comprehensive evidence");
    println!();
    println!("   ❌ DON'T:");
    println!("      - Create commitments you can't fulfill");
    println!("      - Submit low-quality evidence");
    println!("      - Miss deadlines (auto-expires)");
    println!("      - Make vague, unmeasurable commitments");
    println!();

    // 8. Advanced Features
    println!("8. Advanced Features:");
    println!("   - Dispute mechanism: Holders can challenge commitment status");
    println!("   - Appeals process: Issuers can appeal negative decisions");
    println!("   - AI scoring: Automated quality evaluation of outputs");
    println!("   - Fraud detection: Pattern recognition for gaming attempts");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}
