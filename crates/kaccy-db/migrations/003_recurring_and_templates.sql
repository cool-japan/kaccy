-- Recurring commitments for automated periodic deliverables
CREATE TABLE IF NOT EXISTS recurring_commitments (
    recurring_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    title VARCHAR(200) NOT NULL,
    description TEXT,
    frequency VARCHAR(20) NOT NULL CHECK (frequency IN ('weekly', 'biweekly', 'monthly', 'quarterly')),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    instances_created INTEGER NOT NULL DEFAULT 0,
    instances_fulfilled INTEGER NOT NULL DEFAULT 0,
    instances_failed INTEGER NOT NULL DEFAULT 0,
    last_generated_at TIMESTAMPTZ,
    next_deadline TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_recurring_user ON recurring_commitments(user_id);
CREATE INDEX idx_recurring_token ON recurring_commitments(token_id);
CREATE INDEX idx_recurring_active ON recurring_commitments(is_active, next_deadline);

-- Track which commitments came from recurring definitions
CREATE TABLE IF NOT EXISTS recurring_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recurring_id UUID NOT NULL REFERENCES recurring_commitments(recurring_id) ON DELETE CASCADE,
    commitment_id UUID NOT NULL REFERENCES output_commitments(commitment_id) ON DELETE CASCADE,
    instance_number INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(recurring_id, instance_number)
);

CREATE INDEX idx_recurring_instances_recurring ON recurring_instances(recurring_id);
CREATE INDEX idx_recurring_instances_commitment ON recurring_instances(commitment_id);

-- Commitment templates for pre-defined commitment types
CREATE TABLE IF NOT EXISTS commitment_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    category VARCHAR(20) NOT NULL CHECK (category IN ('content', 'development', 'community', 'education', 'events', 'support', 'other')),
    description TEXT NOT NULL,
    default_duration_days INTEGER NOT NULL CHECK (default_duration_days > 0 AND default_duration_days <= 365),
    verification_criteria TEXT NOT NULL,
    evidence_requirements TEXT NOT NULL,
    reputation_reward DECIMAL(10, 2) NOT NULL DEFAULT 10,
    reputation_penalty DECIMAL(10, 2) NOT NULL DEFAULT -20,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    usage_count BIGINT NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_templates_category ON commitment_templates(category);
CREATE INDEX idx_templates_active ON commitment_templates(is_active);
CREATE INDEX idx_templates_usage ON commitment_templates(usage_count DESC);

-- Track which commitments were created from templates
CREATE TABLE IF NOT EXISTS template_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES commitment_templates(template_id),
    commitment_id UUID NOT NULL REFERENCES output_commitments(commitment_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_template_instances_template ON template_instances(template_id);
CREATE INDEX idx_template_instances_commitment ON template_instances(commitment_id);

-- Add metadata column to reputation_events for additional context
ALTER TABLE reputation_events ADD COLUMN IF NOT EXISTS metadata JSONB;
CREATE INDEX IF NOT EXISTS idx_reputation_events_metadata ON reputation_events USING GIN (metadata);
