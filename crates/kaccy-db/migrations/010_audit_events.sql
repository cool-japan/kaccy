-- Migration: Audit Events Table
-- Description: Create a table to store audit events from the event-driven architecture
-- Created: 2026-01-18

-- Create audit_events table
CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_type TEXT NOT NULL,
    event_data JSONB NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'kaccy-api',
    correlation_id TEXT,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    -- Indexes for common queries
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_correlation_id ON audit_events(correlation_id) WHERE correlation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_events_source ON audit_events(source);

-- Create a GIN index on the JSONB event_data for efficient JSON queries
CREATE INDEX IF NOT EXISTS idx_audit_events_event_data ON audit_events USING GIN(event_data);

-- Add partitioning for better performance with large datasets (optional, but recommended)
-- This uses PostgreSQL declarative partitioning by month
-- Uncomment if you want to enable partitioning:
-- ALTER TABLE audit_events RENAME TO audit_events_template;
-- CREATE TABLE audit_events (LIKE audit_events_template INCLUDING ALL) PARTITION BY RANGE (timestamp);
--
-- Create initial partitions (example for a few months)
-- CREATE TABLE audit_events_2024_01 PARTITION OF audit_events
--     FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');
-- CREATE TABLE audit_events_2024_02 PARTITION OF audit_events
--     FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');
-- (add more partitions as needed)

-- Add comment on table
COMMENT ON TABLE audit_events IS 'Stores audit events from the event-driven architecture for compliance and debugging';
COMMENT ON COLUMN audit_events.id IS 'Unique event identifier';
COMMENT ON COLUMN audit_events.timestamp IS 'When the event occurred';
COMMENT ON COLUMN audit_events.event_type IS 'Type/discriminant of the event';
COMMENT ON COLUMN audit_events.event_data IS 'Full event data as JSON';
COMMENT ON COLUMN audit_events.source IS 'Source system that generated the event';
COMMENT ON COLUMN audit_events.correlation_id IS 'Optional correlation ID for distributed tracing';
COMMENT ON COLUMN audit_events.user_id IS 'Associated user ID if applicable';
