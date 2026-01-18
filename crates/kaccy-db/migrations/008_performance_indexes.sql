-- Performance indexes for reputation system
-- Optimizes frequent query patterns identified in codebase analysis

-- Index for reputation_events queries filtered by user_id and event_type
-- Used by: ReputationEngine::fetch_event_stats(), EventService::get_user_events()
CREATE INDEX IF NOT EXISTS idx_reputation_events_user_type_time
ON reputation_events(user_id, event_type, created_at DESC);

-- Index for reputation_events by user_id for event aggregation
-- Used by: EventService::get_event_summary(), EventService::replay_events()
CREATE INDEX IF NOT EXISTS idx_reputation_events_user_created
ON reputation_events(user_id, created_at DESC);

-- Index for users table queries by reputation_score
-- Used by: CircuitBreaker::check_all(), ReputationEngine::get_percentile()
CREATE INDEX IF NOT EXISTS idx_users_reputation_score
ON users(reputation_score DESC, user_id);

-- Index for output_commitments queries by user_id and status
-- Used by: ConfidenceTracker::get_commitment_confidence()
CREATE INDEX IF NOT EXISTS idx_output_commitments_user_status
ON output_commitments(user_id, status, created_at);

-- Index for output_commitments by token_id for token-specific queries
-- Used by: CommitmentRepository::get_token_commitments()
CREATE INDEX IF NOT EXISTS idx_output_commitments_token_id
ON output_commitments(token_id, status, created_at DESC);

-- Index for holder_ratings aggregation queries
-- Used by: HolderRatingService::get_issuer_stats(), calculate_community_trust()
CREATE INDEX IF NOT EXISTS idx_holder_ratings_issuer
ON holder_ratings(issuer_user_id, rating, created_at DESC);

-- Composite index for holder ratings with balance weighting
-- Used by: HolderRatingService::submit_rating() validation
CREATE INDEX IF NOT EXISTS idx_holder_ratings_holder_token
ON holder_ratings(holder_user_id, token_id, created_at DESC);

-- Index for score_snapshots by user_id and timestamp
-- Used by: ScoreHistoryRepository::get_user_history(), get_score_trend()
CREATE INDEX IF NOT EXISTS idx_score_snapshots_user_time
ON score_snapshots(user_id, snapshot_date DESC);

-- Index for tier_history queries
-- Used by: TierHistoryRepository::get_user_history()
CREATE INDEX IF NOT EXISTS idx_tier_history_user_changed
ON tier_history(user_id, changed_at DESC);

-- Index for disputes by status for admin dashboard
-- Used by: DisputeService::get_pending_disputes()
CREATE INDEX IF NOT EXISTS idx_disputes_status_created
ON disputes(status, created_at DESC);

-- Index for dispute_votes by dispute_id for vote aggregation
-- Used by: DisputeService::calculate_results()
CREATE INDEX IF NOT EXISTS idx_dispute_votes_dispute
ON dispute_votes(dispute_id, created_at);

-- Index for milestones by parent commitment
-- Used by: MilestoneService::get_commitment_milestones()
CREATE INDEX IF NOT EXISTS idx_milestones_parent
ON milestones(parent_commitment_id, milestone_number);

-- Index for recurring_commitments by user_id and next_occurrence
-- Used by: RecurringCommitmentService::generate_all_due()
CREATE INDEX IF NOT EXISTS idx_recurring_next_occurrence
ON recurring_commitments(next_occurrence, is_active)
WHERE is_active = true;

-- Index for soulbound_tokens by user_id for quick lookup
-- Used by: SBTService::get_sbt(), has_sbt()
CREATE INDEX IF NOT EXISTS idx_soulbound_tokens_user
ON soulbound_tokens(user_id)
WHERE minted = true;

-- Index for external_reputation_imports by user_id and platform
-- Used by: PortabilityService::get_verified_imports()
CREATE INDEX IF NOT EXISTS idx_external_reputation_user_platform
ON external_reputation_imports(user_id, platform, verified);

-- Index for fraud_detections by user_id for fraud check queries
-- Used by: AIScoringService::get_fraud_detections()
CREATE INDEX IF NOT EXISTS idx_fraud_detections_user_detected
ON fraud_detections(user_id, detected_at DESC);

-- Partial index for active commitment templates
-- Used by: TemplateService::get_active_templates()
CREATE INDEX IF NOT EXISTS idx_templates_active_category
ON commitment_templates(category, created_at DESC)
WHERE is_active = true;

-- Index for admin_actions audit trail
-- Used by: Audit queries and compliance reporting
CREATE INDEX IF NOT EXISTS idx_admin_actions_user_created
ON admin_actions(user_id, created_at DESC);

-- Index for admin_actions by admin_user_id for admin activity tracking
CREATE INDEX IF NOT EXISTS idx_admin_actions_admin
ON admin_actions(admin_user_id, created_at DESC);

-- Add comments for maintenance
COMMENT ON INDEX idx_reputation_events_user_type_time IS 'Optimizes event filtering by user and type';
COMMENT ON INDEX idx_users_reputation_score IS 'Optimizes circuit breaker and percentile queries';
COMMENT ON INDEX idx_holder_ratings_issuer IS 'Optimizes community trust calculations';
COMMENT ON INDEX idx_recurring_next_occurrence IS 'Partial index for active recurring commitments due soon';
COMMENT ON INDEX idx_soulbound_tokens_user IS 'Partial index for minted SBTs only';
