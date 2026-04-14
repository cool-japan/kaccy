# kaccy-reputation

Comprehensive reputation system for the Kaccy Protocol, providing on-chain and off-chain reputation tracking, AI-assisted scoring, dispute resolution, and cross-platform portability.

**Version:** 0.2.0 | **Tests:** 489 passing

## Features

### Phase 0 (MVP) - Core Reputation

#### Reputation Scoring
- **Score Components**: Multi-dimensional scoring (commitment fulfillment, response time, quality rating, community trust, longevity)
- **Weighted Calculation**: Configurable weights for each component
- **Score Bounds**: 0-1000 range with delta validation (max ±50 positive, ±100 negative per update)
- **Event-Based**: All score changes tracked as events for auditability

#### Tier System
- **6 Tiers**: Unverified, Bronze, Silver, Gold, Platinum, Diamond
- **Tier Benefits**: Dynamic benefits based on tier (fee discounts, token issuance limits, priority support)
- **Tier History**: Complete tracking of tier changes with timestamps

#### Commitment Tracking
- **Commitment Lifecycle**: Pending → Completed → Verified/Failed
- **Evidence Submission**: Users submit evidence, admins verify
- **Expiration Handling**: Auto-expire overdue commitments

#### Circuit Breaker
- **Low Reputation Protection**: Auto-disable features when score < 200
- **Automatic Recovery**: Restore privileges when score improves
- **Admin Notifications**: Alert admins on circuit breaker triggers

### Phase 1 (Post-MVP) - Advanced Features

#### Advanced Scoring
- **Component Confidence**: Track confidence levels (VeryLow to VeryHigh) based on data availability
- **Time Decay**: Recent events weighted more heavily, inactivity decay after 90 days
- **Score History**: Daily snapshots with trend analysis
- **Percentile Ranking**: Compare users across the platform

#### Commitment Enhancements
- **Milestone Commitments**: Break large commitments into trackable milestones
- **Recurring Commitments**: Auto-generate periodic deliverables (weekly, monthly, quarterly)
- **Commitment Templates**: 15+ pre-defined templates for common commitment types

#### Holder Ratings
- **Token Holder Ratings**: 1-5 star rating system weighted by holdings
- **Community Trust**: Aggregate holder ratings into reputation score
- **Minimum Requirements**: Balance and duration requirements for rating eligibility

#### Event System
- **Comprehensive Event Types**: commitment_fulfilled, commitment_failed, holder_rating, quality_score, admin_adjustment, time_decay, circuit_breaker
- **Event Replay**: Recalculate scores from event history
- **Notifications**: Multi-severity notifications (Info, Warning, Critical)

### Phase 2 - Future-Ready Features

#### Soulbound Tokens (SBT)
- **On-Chain Reputation**: Mint non-transferable reputation tokens
- **Dynamic Metadata**: Auto-update metadata with score changes
- **Cross-Platform**: Use reputation on other platforms via metadata URI
- **Blockchain Integration**: `OnChainSBT` trait for multiple blockchain support

```rust
use kaccy_reputation::{SBTService, MintSBTRequest};

let sbt_service = SBTService::new(pool);
let request = MintSBTRequest {
    user_id,
    initial_score: 750,
    initial_tier: ReputationTier::Gold,
};
let sbt = sbt_service.create_sbt(request).await?;
```

#### AI-Assisted Scoring
- **Quality Evaluation**: Automatic assessment of deliverable quality with aspect-based scoring
- **Fraud Detection**: Detect sybil attacks, score farming, vote manipulation
- **Success Prediction**: Predict commitment success probability using historical data

```rust
use kaccy_reputation::{AIScoringService, EvaluateQualityRequest};

let ai_service = AIScoringService::new(pool);
let evaluation = ai_service.evaluate_quality(request).await?;
println!("Quality score: {}", evaluation.quality_score);
```

#### Dispute Resolution
- **DAO Voting**: Token-weighted voting on disputed commitments
- **Quorum Requirements**: Configurable voting thresholds
- **Appeals Process**: Time-limited appeals with admin review

```rust
use kaccy_reputation::{DisputeService, CreateDisputeRequest, VoteDisputeRequest};

let dispute_service = DisputeService::new(pool);
let dispute = dispute_service.create_dispute(request).await?;
let vote = dispute_service.vote(vote_request).await?;
let results = dispute_service.calculate_results(dispute.id).await?;
```

#### Reputation Portability
- **Import**: GitHub, LinkedIn, Stack Overflow, Twitter, Discord
- **Score Calculation**: Platform-specific algorithms for fair conversion
- **Export**: W3C Verifiable Credentials with cryptographic proofs
- **Interoperability**: Use Kaccy reputation on other platforms

```rust
use kaccy_reputation::{PortabilityService, ImportReputationRequest, Platform};

let portability = PortabilityService::new(pool);
let result = portability.import_reputation(ImportReputationRequest {
    user_id,
    platform: Platform::GitHub,
    platform_user_id: "username".to_string(),
    access_token: Some(token),
}).await?;

let credential = portability.export_reputation(ExportReputationRequest {
    user_id,
    include_proof: true,
    expiration_days: Some(365),
}).await?;
```

### Phase 23 - Ergonomic Enhancements (v0.2.0)

- **Score Comparison**: Side-by-side comparison of two users' reputation profiles
- **Component Analysis**: Drill-down breakdown of each score component with confidence levels
- **Tier Progression**: Compute score delta needed to reach the next tier and projected timeline
- **Batch Processing**: High-throughput score updates via `batch.rs`
- **Leaderboard**: Real-time top-N reputation rankings via `leaderboard.rs`
- **Validation Layer**: Centralized input validation for all public APIs via `validation.rs`

### Integration

The `ReputationOrchestrator` provides high-level workflows coordinating all features:

```rust
use kaccy_reputation::ReputationOrchestrator;

let orchestrator = ReputationOrchestrator::new(pool);

// Complete workflow: First token → SBT minting
orchestrator.on_first_token_issuance(user_id).await?;

// Complete workflow: Commitment → AI eval → Score update → SBT update
let quality_score = orchestrator.evaluate_and_score_commitment(
    commitment_id,
    user_id,
    evidence_url,
    evidence_description,
    commitment_description,
).await?;

// Complete profile with all features
let profile = orchestrator.get_complete_profile(user_id).await?;
println!("Score: {}, Tier: {:?}, Clean: {}",
    profile.score, profile.tier, profile.clean_record);
```

## Score Components

| Component | Weight | Description |
|-----------|--------|-------------|
| Commitment Fulfillment | 35% | Percentage of commitments successfully completed |
| Response Time | 15% | How quickly issuer responds to holders |
| Quality Rating | 30% | Average quality score of delivered outputs |
| Community Trust | 15% | Ratings from token holders |
| Longevity | 5% | Account age and activity consistency |

## Tier System

| Tier | Score Range | Benefits |
|------|-------------|----------|
| Diamond | 900-1000 | 5 tokens, 50% fee discount, verified badge |
| Platinum | 800-899 | 3 tokens, 30% fee discount, verified badge |
| Gold | 600-799 | 2 tokens, 15% fee discount, verified badge |
| Silver | 400-599 | 1 token, 5% fee discount |
| Bronze | 200-399 | 1 token, no fee discount, cannot issue |
| Unverified | 0-199 | Cannot issue tokens (circuit breaker) |

## Architecture

### Modules

```
kaccy-reputation/
├── src/
│   ├── lib.rs              # Crate root, re-exports
│   ├── score.rs            # Core scoring engine
│   ├── tier.rs             # Tier definitions & benefits
│   ├── commitment.rs       # Commitment tracking
│   ├── circuit_breaker.rs  # Low reputation protection
│   ├── confidence.rs       # Component confidence tracking
│   ├── time_decay.rs       # Time-based score decay
│   ├── score_history.rs    # Historical snapshots
│   ├── history.rs          # Tier change history
│   ├── milestone.rs        # Milestone commitments
│   ├── recurring.rs        # Recurring commitments
│   ├── templates.rs        # Commitment templates (15+ types)
│   ├── holder_ratings.rs   # Token holder ratings
│   ├── events.rs           # Event sourcing & notifications
│   ├── sbt.rs              # Soulbound tokens
│   ├── ai_scoring.rs       # AI evaluations & fraud detection
│   ├── dispute.rs          # Dispute resolution & appeals
│   ├── portability.rs      # Import/export reputation
│   ├── integration.rs      # ReputationOrchestrator (high-level workflows)
│   ├── query_builder.rs    # Dynamic reputation query construction
│   ├── statistics.rs       # Aggregate statistics & percentile ranking
│   ├── cache.rs            # Reputation score caching layer
│   ├── analytics.rs        # Trend analysis & scoring insights
│   ├── audit.rs            # Audit trails for score changes
│   ├── batch.rs            # Batch score update processing
│   ├── leaderboard.rs      # Top-reputation leaderboards
│   ├── maintenance.rs      # Score maintenance & cleanup routines
│   ├── export_import.rs    # Bulk data export/import
│   ├── validation.rs       # Input validation for reputation data
│   └── error.rs            # Error types
└── Cargo.toml
```

### Database Schema

See `../kaccy-db/migrations/` for complete schema:
- `001_initial_schema.sql` - Core tables
- `002_score_snapshots.sql` - Score history
- `003_recurring_and_templates.sql` - Commitment enhancements
- `004_component_confidence.sql` - Confidence tracking
- `005_audit_logs.sql` - Audit trails
- `006_phase2_tables.sql` - SBT, AI, disputes, portability

## Usage Examples

### Basic Reputation Tracking

```rust
use kaccy_reputation::{ReputationEngine, ReputationEventType};
use rust_decimal_macros::dec;

let engine = ReputationEngine::new(pool);

// Get current score
let score = engine.get_score(user_id).await?;
println!("Score: {}, Tier: {:?}", score.overall_score, score.tier);

// Update score with event
let new_score = engine.update_score_with_event(
    user_id,
    dec!(25),
    ReputationEventType::CommitmentFulfilled,
    Some("Delivered on time"),
).await?;
```

### Commitment Management

```rust
use kaccy_reputation::{MilestoneService, RecurringCommitmentService};

// Milestone commitment
let milestone_svc = MilestoneService::new(pool);
let commitment = milestone_svc.create_commitment(request).await?;
milestone_svc.complete_milestone(commitment.id, 0).await?;

// Recurring commitment
let recurring_svc = RecurringCommitmentService::new(pool);
let recurring = recurring_svc.create(request).await?;
recurring_svc.generate_all_due().await?;
```

### Holder Ratings

```rust
use kaccy_reputation::{HolderRatingService, RatingRequest};

let rating_svc = HolderRatingService::new(pool);
let rating = rating_svc.create_rating(RatingRequest {
    token_id,
    holder_id,
    issuer_id,
    rating: 4,
    comment: Some("Great project!".to_string()),
}).await?;

let stats = rating_svc.get_stats(issuer_id).await?;
```

## Error Handling

All operations return `Result<T, ReputationError>`:

```rust
match engine.get_score(user_id).await {
    Ok(score) => println!("Score: {}", score.overall_score),
    Err(ReputationError::UserNotFound(id)) => println!("User {} not found", id),
    Err(e) => println!("Error: {:?}", e),
}
```

## Testing

The crate ships with **489 passing tests** covering all scoring, tier, commitment, AI, dispute, portability, analytics, and orchestration modules.

```bash
# Run all tests
cargo test -p kaccy-reputation

# Run with nextest
cargo nextest run -p kaccy-reputation --no-fail-fast

# Run with output
cargo test -p kaccy-reputation -- --nocapture
```

## Dependencies

- `rust_decimal` - Precise score calculations
- `sqlx` - Database queries
- `chrono` - Deadline handling
- `serde` / `serde_json` - Serialization
- `uuid` - Unique identifiers
- `tokio` - Async runtime
- `async-trait` - Async traits
- `thiserror` - Error definitions

## License

See LICENSE file in repository root.
