-- Score snapshots for reputation history tracking
CREATE TABLE IF NOT EXISTS score_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    snapshot_date DATE NOT NULL,
    score DECIMAL(10, 2) NOT NULL,
    tier VARCHAR(20) NOT NULL,
    delta_from_previous DECIMAL(10, 2) NOT NULL DEFAULT 0,
    components_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, snapshot_date)
);

CREATE INDEX idx_score_snapshots_user ON score_snapshots(user_id);
CREATE INDEX idx_score_snapshots_date ON score_snapshots(snapshot_date);
CREATE INDEX idx_score_snapshots_user_date ON score_snapshots(user_id, snapshot_date DESC);

-- Token metadata for extended token information
CREATE TABLE IF NOT EXISTS token_metadata (
    token_id UUID PRIMARY KEY REFERENCES tokens(token_id),
    cover_image_url VARCHAR(500),
    website_url VARCHAR(500),
    twitter_handle VARCHAR(50),
    discord_url VARCHAR(500),
    github_url VARCHAR(500),
    youtube_url VARCHAR(500),
    telegram_url VARCHAR(500),
    tags VARCHAR(500),
    faq_content TEXT,
    terms_of_service TEXT,
    has_utilities BOOLEAN NOT NULL DEFAULT FALSE,
    utility_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Utility definitions for token holder benefits
CREATE TABLE IF NOT EXISTS utility_definitions (
    utility_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    utility_type VARCHAR(50) NOT NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    min_tokens_required DECIMAL(20, 8) NOT NULL DEFAULT 0,
    max_redemptions_per_holder INTEGER,
    max_total_redemptions INTEGER,
    total_redemptions_used INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    redemption_instructions TEXT,
    redemption_url VARCHAR(500),
    estimated_value_btc DECIMAL(20, 8),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_utility_definitions_token ON utility_definitions(token_id);
CREATE INDEX idx_utility_definitions_active ON utility_definitions(token_id, is_active);

-- Utility redemptions for tracking holder usage
CREATE TABLE IF NOT EXISTS utility_redemptions (
    redemption_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    utility_id UUID NOT NULL REFERENCES utility_definitions(utility_id),
    user_id UUID NOT NULL REFERENCES users(user_id),
    tokens_held DECIMAL(20, 8) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    notes TEXT,
    redeemed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    fulfilled_at TIMESTAMPTZ
);

CREATE INDEX idx_utility_redemptions_utility ON utility_redemptions(utility_id);
CREATE INDEX idx_utility_redemptions_user ON utility_redemptions(user_id);
CREATE INDEX idx_utility_redemptions_status ON utility_redemptions(status);

-- Tier history for tracking reputation tier changes
CREATE TABLE IF NOT EXISTS tier_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    previous_tier VARCHAR(20) NOT NULL,
    new_tier VARCHAR(20) NOT NULL,
    previous_score DECIMAL(10, 2) NOT NULL,
    new_score DECIMAL(10, 2) NOT NULL,
    change_reason VARCHAR(200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tier_history_user ON tier_history(user_id);
CREATE INDEX idx_tier_history_created_at ON tier_history(created_at);

-- Holder ratings for community trust component
CREATE TABLE IF NOT EXISTS holder_ratings (
    rating_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_user_id UUID NOT NULL REFERENCES users(user_id),
    holder_user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    rating SMALLINT NOT NULL CHECK (rating >= 1 AND rating <= 5),
    feedback TEXT,
    holder_balance DECIMAL(20, 8) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(holder_user_id, token_id)
);

CREATE INDEX idx_holder_ratings_issuer ON holder_ratings(issuer_user_id);
CREATE INDEX idx_holder_ratings_token ON holder_ratings(token_id);
CREATE INDEX idx_holder_ratings_created_at ON holder_ratings(created_at);

-- Commitment milestones for breaking large commitments into stages
CREATE TABLE IF NOT EXISTS commitment_milestones (
    milestone_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commitment_id UUID NOT NULL REFERENCES output_commitments(commitment_id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    deadline TIMESTAMPTZ NOT NULL,
    weight_percent DECIMAL(5, 2) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    evidence_url VARCHAR(500),
    evidence_description TEXT,
    verified_at TIMESTAMPTZ,
    verified_by UUID REFERENCES users(user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(commitment_id, sequence_number)
);

CREATE INDEX idx_commitment_milestones_commitment ON commitment_milestones(commitment_id);
CREATE INDEX idx_commitment_milestones_status ON commitment_milestones(status);
CREATE INDEX idx_commitment_milestones_deadline ON commitment_milestones(deadline);
