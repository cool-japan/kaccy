-- Migration 006: Phase 2 Tables (SBT, AI Scoring, Disputes, Portability)
-- Created: 2024-12-12

-- =============================================================================
-- Soulbound Tokens (SBT)
-- =============================================================================

CREATE TABLE IF NOT EXISTS soulbound_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id TEXT,  -- On-chain token ID after minting
    score INTEGER NOT NULL,
    tier TEXT NOT NULL,
    metadata_uri TEXT NOT NULL,
    minted BOOLEAN NOT NULL DEFAULT false,
    mint_tx_hash TEXT,
    last_metadata_update TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id)
);

CREATE INDEX idx_sbt_user_id ON soulbound_tokens(user_id);
CREATE INDEX idx_sbt_minted ON soulbound_tokens(minted);
CREATE INDEX idx_sbt_token_id ON soulbound_tokens(token_id) WHERE token_id IS NOT NULL;

COMMENT ON TABLE soulbound_tokens IS 'Non-transferable on-chain reputation tokens';
COMMENT ON COLUMN soulbound_tokens.token_id IS 'Blockchain token ID after minting';
COMMENT ON COLUMN soulbound_tokens.metadata_uri IS 'URI pointing to JSON metadata';

-- =============================================================================
-- AI Quality Evaluations
-- =============================================================================

CREATE TABLE IF NOT EXISTS ai_quality_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES commitments(commitment_id),
    quality_score INTEGER NOT NULL CHECK (quality_score >= 0 AND quality_score <= 100),
    confidence INTEGER NOT NULL CHECK (confidence >= 0 AND confidence <= 100),
    explanation TEXT NOT NULL,
    evaluated_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quality_eval_commitment ON ai_quality_evaluations(commitment_id);
CREATE INDEX idx_quality_eval_created ON ai_quality_evaluations(created_at DESC);

COMMENT ON TABLE ai_quality_evaluations IS 'AI-powered quality evaluations of commitment deliverables';
COMMENT ON COLUMN ai_quality_evaluations.quality_score IS 'Overall quality score (0-100)';
COMMENT ON COLUMN ai_quality_evaluations.confidence IS 'AI confidence level (0-100)';

-- =============================================================================
-- Fraud Detection
-- =============================================================================

CREATE TABLE IF NOT EXISTS fraud_detections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    detected_at TIMESTAMPTZ NOT NULL,
    risk_score INTEGER NOT NULL CHECK (risk_score >= 0 AND risk_score <= 100),
    pattern_type TEXT NOT NULL,
    evidence JSONB NOT NULL,
    recommended_action TEXT NOT NULL,
    reviewed BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fraud_user_id ON fraud_detections(user_id);
CREATE INDEX idx_fraud_reviewed ON fraud_detections(reviewed) WHERE reviewed = false;
CREATE INDEX idx_fraud_risk_score ON fraud_detections(risk_score DESC);
CREATE INDEX idx_fraud_created ON fraud_detections(created_at DESC);

COMMENT ON TABLE fraud_detections IS 'AI-detected fraudulent behavior patterns';
COMMENT ON COLUMN fraud_detections.risk_score IS 'Fraud risk level (0-100, higher = more suspicious)';
COMMENT ON COLUMN fraud_detections.pattern_type IS 'Type of fraud pattern detected';

-- =============================================================================
-- Disputes
-- =============================================================================

CREATE TABLE IF NOT EXISTS disputes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES commitments(commitment_id),
    disputer_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    reason TEXT NOT NULL,
    evidence TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    voting_deadline TIMESTAMPTZ NOT NULL,
    resolution TEXT,
    resolved_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'voting', 'upheld', 'rejected', 'withdrawn', 'expired'))
);

CREATE INDEX idx_dispute_commitment ON disputes(commitment_id);
CREATE INDEX idx_dispute_disputer ON disputes(disputer_id);
CREATE INDEX idx_dispute_token ON disputes(token_id);
CREATE INDEX idx_dispute_status ON disputes(status);
CREATE INDEX idx_dispute_voting_deadline ON disputes(voting_deadline);

COMMENT ON TABLE disputes IS 'Holder disputes challenging commitment verifications';
COMMENT ON COLUMN disputes.voting_deadline IS 'When voting period ends';

-- =============================================================================
-- Dispute Votes
-- =============================================================================

CREATE TABLE IF NOT EXISTS dispute_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dispute_id UUID NOT NULL REFERENCES disputes(id) ON DELETE CASCADE,
    voter_id UUID NOT NULL REFERENCES users(user_id),
    vote TEXT NOT NULL,
    voting_power DECIMAL(20, 8) NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (vote IN ('uphold', 'reject', 'abstain')),
    UNIQUE(dispute_id, voter_id)
);

CREATE INDEX idx_vote_dispute ON dispute_votes(dispute_id);
CREATE INDEX idx_vote_voter ON dispute_votes(voter_id);

COMMENT ON TABLE dispute_votes IS 'DAO votes on disputes';
COMMENT ON COLUMN dispute_votes.voting_power IS 'Voting power based on token holdings';

-- =============================================================================
-- Appeals
-- =============================================================================

CREATE TABLE IF NOT EXISTS appeals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES commitments(commitment_id),
    issuer_id UUID NOT NULL REFERENCES users(user_id),
    reason TEXT NOT NULL,
    additional_evidence TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    appeal_deadline TIMESTAMPTZ NOT NULL,
    resolution TEXT,
    resolved_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'under_review', 'granted', 'denied', 'withdrawn', 'expired'))
);

CREATE INDEX idx_appeal_commitment ON appeals(commitment_id);
CREATE INDEX idx_appeal_issuer ON appeals(issuer_id);
CREATE INDEX idx_appeal_status ON appeals(status);
CREATE INDEX idx_appeal_created ON appeals(created_at DESC);

COMMENT ON TABLE appeals IS 'Issuer appeals against negative decisions';
COMMENT ON COLUMN appeals.appeal_deadline IS 'Deadline to file appeal';

-- =============================================================================
-- External Reputations
-- =============================================================================

CREATE TABLE IF NOT EXISTS external_reputations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    platform TEXT NOT NULL,
    platform_user_id TEXT NOT NULL,
    reputation_data JSONB NOT NULL,
    imported_score INTEGER NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, platform, platform_user_id)
);

CREATE INDEX idx_external_rep_user ON external_reputations(user_id);
CREATE INDEX idx_external_rep_platform ON external_reputations(platform);
CREATE INDEX idx_external_rep_verified ON external_reputations(verified);

COMMENT ON TABLE external_reputations IS 'Imported reputation from external platforms';
COMMENT ON COLUMN external_reputations.platform IS 'Platform type (github, linkedin, stackoverflow, etc.)';
COMMENT ON COLUMN external_reputations.imported_score IS 'Calculated score from external data';

-- =============================================================================
-- Reputation Exports
-- =============================================================================

CREATE TABLE IF NOT EXISTS reputation_exports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    credential_data JSONB NOT NULL,
    exported_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rep_export_user ON reputation_exports(user_id);
CREATE INDEX idx_rep_export_date ON reputation_exports(exported_at DESC);

COMMENT ON TABLE reputation_exports IS 'Verifiable credential exports for portability';
COMMENT ON COLUMN reputation_exports.credential_data IS 'W3C verifiable credential JSON';

-- =============================================================================
-- Update Triggers
-- =============================================================================

-- Trigger to update updated_at on soulbound_tokens
CREATE OR REPLACE FUNCTION update_sbt_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sbt_updated_at
    BEFORE UPDATE ON soulbound_tokens
    FOR EACH ROW
    EXECUTE FUNCTION update_sbt_timestamp();

-- Trigger to update updated_at on disputes
CREATE OR REPLACE FUNCTION update_dispute_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER dispute_updated_at
    BEFORE UPDATE ON disputes
    FOR EACH ROW
    EXECUTE FUNCTION update_dispute_timestamp();

-- Trigger to update updated_at on appeals
CREATE OR REPLACE FUNCTION update_appeal_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER appeal_updated_at
    BEFORE UPDATE ON appeals
    FOR EACH ROW
    EXECUTE FUNCTION update_appeal_timestamp();

-- Trigger to update updated_at on external_reputations
CREATE OR REPLACE FUNCTION update_external_rep_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER external_rep_updated_at
    BEFORE UPDATE ON external_reputations
    FOR EACH ROW
    EXECUTE FUNCTION update_external_rep_timestamp();
