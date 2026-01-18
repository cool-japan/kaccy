-- System Configuration Management
-- Supports dynamic configuration with versioning, validation, and audit trail

CREATE TABLE system_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_key VARCHAR(255) NOT NULL,
    config_value TEXT NOT NULL,
    value_type VARCHAR(50) NOT NULL DEFAULT 'string', -- string, number, boolean, json

    -- Metadata
    description TEXT,
    category VARCHAR(100), -- feature_flags, api_limits, ui_settings, etc.
    environment VARCHAR(50) DEFAULT 'production', -- production, staging, development, all

    -- Versioning
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT true,

    -- Validation
    validation_rules JSONB, -- JSON schema or validation rules
    is_sensitive BOOLEAN NOT NULL DEFAULT false, -- Whether value contains sensitive data

    -- Change tracking
    changed_by UUID, -- User who made the change
    changed_reason TEXT, -- Reason for the change
    previous_value TEXT, -- Previous value for rollback

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    effective_from TIMESTAMPTZ, -- When this config becomes active
    effective_until TIMESTAMPTZ, -- When this config expires

    CONSTRAINT fk_changed_by FOREIGN KEY (changed_by) REFERENCES users(user_id) ON DELETE SET NULL
);

-- Unique constraint for active configs per environment
CREATE UNIQUE INDEX idx_system_config_key_env_active
    ON system_config(config_key, environment)
    WHERE is_active = true;

-- Index for fetching active configs
CREATE INDEX idx_system_config_active
    ON system_config(is_active, environment)
    WHERE is_active = true;

-- Index for category filtering
CREATE INDEX idx_system_config_category
    ON system_config(category, is_active);

-- Index for change tracking and audit
CREATE INDEX idx_system_config_changed_by
    ON system_config(changed_by, updated_at DESC)
    WHERE changed_by IS NOT NULL;

-- Index for version history
CREATE INDEX idx_system_config_key_version
    ON system_config(config_key, version DESC);

-- Index for time-based configs
CREATE INDEX idx_system_config_effective
    ON system_config(effective_from, effective_until)
    WHERE effective_from IS NOT NULL OR effective_until IS NOT NULL;

-- Configuration change history table (for audit trail)
CREATE TABLE system_config_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id UUID NOT NULL,
    config_key VARCHAR(255) NOT NULL,
    old_value TEXT,
    new_value TEXT NOT NULL,
    value_type VARCHAR(50) NOT NULL,
    change_type VARCHAR(50) NOT NULL, -- created, updated, deleted, activated, deactivated
    changed_by UUID,
    changed_reason TEXT,
    environment VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_config FOREIGN KEY (config_id) REFERENCES system_config(id) ON DELETE CASCADE,
    CONSTRAINT fk_history_changed_by FOREIGN KEY (changed_by) REFERENCES users(user_id) ON DELETE SET NULL
);

-- Index for config history lookup
CREATE INDEX idx_system_config_history_config
    ON system_config_history(config_id, created_at DESC);

-- Index for user audit trail
CREATE INDEX idx_system_config_history_user
    ON system_config_history(changed_by, created_at DESC)
    WHERE changed_by IS NOT NULL;

-- Index for change type analysis
CREATE INDEX idx_system_config_history_change_type
    ON system_config_history(change_type, created_at DESC);

COMMENT ON TABLE system_config IS 'Dynamic system configuration with versioning and audit trail';
COMMENT ON COLUMN system_config.config_key IS 'Unique configuration key (e.g., max_api_requests_per_hour)';
COMMENT ON COLUMN system_config.value_type IS 'Data type: string, number, boolean, json';
COMMENT ON COLUMN system_config.category IS 'Configuration category for organization';
COMMENT ON COLUMN system_config.environment IS 'Environment: production, staging, development, or all';
COMMENT ON COLUMN system_config.version IS 'Version number for tracking changes';
COMMENT ON COLUMN system_config.validation_rules IS 'JSON schema or validation rules for the value';
COMMENT ON COLUMN system_config.is_sensitive IS 'Whether value contains sensitive data (encrypted storage recommended)';
COMMENT ON COLUMN system_config.effective_from IS 'When this configuration becomes active (scheduled configs)';
COMMENT ON COLUMN system_config.effective_until IS 'When this configuration expires';

COMMENT ON TABLE system_config_history IS 'Audit trail of all configuration changes';
COMMENT ON COLUMN system_config_history.change_type IS 'Type of change: created, updated, deleted, activated, deactivated';
