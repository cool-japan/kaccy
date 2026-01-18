-- User Session Management
-- Tracks active and historical user sessions with device and security information

CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    session_token VARCHAR(255) NOT NULL UNIQUE, -- Hashed session token

    -- Device information
    device_type VARCHAR(50), -- mobile, desktop, tablet, etc.
    device_name VARCHAR(255), -- User agent or device name
    os VARCHAR(100), -- Operating system
    browser VARCHAR(100), -- Browser name and version

    -- Network information
    ip_address INET NOT NULL,
    country VARCHAR(2), -- ISO country code
    city VARCHAR(100),

    -- Session lifecycle
    logged_in_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    logged_out_at TIMESTAMPTZ, -- NULL if still active

    -- Security
    is_active BOOLEAN NOT NULL DEFAULT true,
    logout_reason VARCHAR(50), -- manual, expired, revoked, security
    security_flags JSONB, -- Additional security metadata

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

-- Index for finding active sessions for a user
CREATE INDEX idx_user_sessions_user_active
    ON user_sessions(user_id, is_active)
    WHERE is_active = true;

-- Index for session token lookup (authentication)
CREATE INDEX idx_user_sessions_token
    ON user_sessions(session_token)
    WHERE is_active = true AND logged_out_at IS NULL;

-- Index for finding expired sessions (cleanup)
CREATE INDEX idx_user_sessions_expires
    ON user_sessions(expires_at)
    WHERE is_active = true AND logged_out_at IS NULL;

-- Index for session activity monitoring
CREATE INDEX idx_user_sessions_last_activity
    ON user_sessions(user_id, last_activity_at DESC)
    WHERE is_active = true;

-- Index for IP-based security analysis
CREATE INDEX idx_user_sessions_ip
    ON user_sessions(ip_address, logged_in_at DESC);

-- Index for device tracking
CREATE INDEX idx_user_sessions_device
    ON user_sessions(user_id, device_type, logged_in_at DESC);

-- Index for historical session queries
CREATE INDEX idx_user_sessions_logged_in
    ON user_sessions(user_id, logged_in_at DESC);

COMMENT ON TABLE user_sessions IS 'User session tracking with device and security information';
COMMENT ON COLUMN user_sessions.session_token IS 'Hashed session token for authentication';
COMMENT ON COLUMN user_sessions.device_type IS 'Device type: mobile, desktop, tablet';
COMMENT ON COLUMN user_sessions.ip_address IS 'IP address at login time';
COMMENT ON COLUMN user_sessions.is_active IS 'Whether session is currently active';
COMMENT ON COLUMN user_sessions.logout_reason IS 'Reason for logout: manual, expired, revoked, security';
COMMENT ON COLUMN user_sessions.security_flags IS 'Additional security metadata (2FA status, suspicious activity, etc.)';
