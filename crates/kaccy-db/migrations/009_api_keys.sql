-- Migration 009: API Keys for Authentication
-- Created: 2026-01-18

-- =============================================================================
-- API Keys
-- =============================================================================

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,  -- First 8 chars for identification
    name TEXT NOT NULL,
    scopes JSONB NOT NULL DEFAULT '[]'::jsonb,  -- Array of permission scopes
    rate_limit_per_hour INTEGER,  -- NULL = use default rate limit
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_key_user ON api_keys(user_id);
CREATE INDEX idx_api_key_prefix ON api_keys(key_prefix);
CREATE INDEX idx_api_key_active ON api_keys(user_id)
    WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW());

COMMENT ON TABLE api_keys IS 'API keys for programmatic access';
COMMENT ON COLUMN api_keys.key_hash IS 'Argon2 hash of the API key';
COMMENT ON COLUMN api_keys.key_prefix IS 'First 8 characters for key identification';
COMMENT ON COLUMN api_keys.scopes IS 'JSON array of allowed scopes (e.g., ["tokens:read", "orders:write"])';
COMMENT ON COLUMN api_keys.rate_limit_per_hour IS 'Custom rate limit override for this key';

-- =============================================================================
-- API Key Usage Tracking
-- =============================================================================

CREATE TABLE IF NOT EXISTS api_key_usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key_id UUID NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL,
    method TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    response_time_ms INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_usage_key ON api_key_usage(api_key_id);
CREATE INDEX idx_api_usage_created ON api_key_usage(created_at DESC);
CREATE INDEX idx_api_usage_key_time ON api_key_usage(api_key_id, created_at DESC);

COMMENT ON TABLE api_key_usage IS 'API key usage statistics and analytics';

-- =============================================================================
-- Update Trigger
-- =============================================================================

CREATE OR REPLACE FUNCTION update_api_key_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER api_key_updated_at
    BEFORE UPDATE ON api_keys
    FOR EACH ROW
    EXECUTE FUNCTION update_api_key_timestamp();
