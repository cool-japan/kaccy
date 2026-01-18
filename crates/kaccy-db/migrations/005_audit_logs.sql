-- Add audit logging tables and triggers for compliance and security tracking
--
-- This migration creates:
-- 1. audit_logs table for recording all significant changes
-- 2. Triggers for automatic change tracking on critical tables
-- 3. Helper functions for audit logging

-- Create audit_logs table
CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    table_name TEXT NOT NULL,
    record_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'DELETE')),
    user_id UUID,
    old_values JSONB,
    new_values JSONB,
    changed_fields TEXT[],
    ip_address TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX idx_audit_logs_table_name ON audit_logs(table_name);
CREATE INDEX idx_audit_logs_record_id ON audit_logs(record_id);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_table_record ON audit_logs(table_name, record_id);

-- Create audit trigger function
CREATE OR REPLACE FUNCTION audit_trigger_func()
RETURNS TRIGGER AS $$
DECLARE
    old_data JSONB;
    new_data JSONB;
    changed_fields TEXT[];
    audit_user_id UUID;
BEGIN
    -- Get user_id from session if available
    BEGIN
        audit_user_id := current_setting('app.current_user_id', TRUE)::UUID;
    EXCEPTION WHEN OTHERS THEN
        audit_user_id := NULL;
    END;

    -- Handle different operation types
    IF (TG_OP = 'DELETE') THEN
        old_data := to_jsonb(OLD);
        new_data := NULL;
        changed_fields := NULL;
    ELSIF (TG_OP = 'UPDATE') THEN
        old_data := to_jsonb(OLD);
        new_data := to_jsonb(NEW);

        -- Find changed fields
        SELECT array_agg(key)
        INTO changed_fields
        FROM jsonb_each(new_data)
        WHERE new_data->key IS DISTINCT FROM old_data->key;

        -- Skip audit if nothing changed
        IF changed_fields IS NULL OR array_length(changed_fields, 1) = 0 THEN
            RETURN NEW;
        END IF;
    ELSIF (TG_OP = 'INSERT') THEN
        old_data := NULL;
        new_data := to_jsonb(NEW);
        changed_fields := NULL;
    END IF;

    -- Insert audit log
    INSERT INTO audit_logs (
        table_name,
        record_id,
        action,
        user_id,
        old_values,
        new_values,
        changed_fields
    ) VALUES (
        TG_TABLE_NAME,
        COALESCE(NEW.id, OLD.id),
        TG_OP,
        audit_user_id,
        old_data,
        new_data,
        changed_fields
    );

    IF (TG_OP = 'DELETE') THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for critical tables
DROP TRIGGER IF EXISTS audit_users ON users;
CREATE TRIGGER audit_users
    AFTER INSERT OR UPDATE OR DELETE ON users
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_func();

DROP TRIGGER IF EXISTS audit_tokens ON tokens;
CREATE TRIGGER audit_tokens
    AFTER INSERT OR UPDATE OR DELETE ON tokens
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_func();

DROP TRIGGER IF EXISTS audit_balances ON balances;
CREATE TRIGGER audit_balances
    AFTER INSERT OR UPDATE OR DELETE ON balances
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_func();

DROP TRIGGER IF EXISTS audit_orders ON orders;
CREATE TRIGGER audit_orders
    AFTER INSERT OR UPDATE OR DELETE ON orders
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_func();

DROP TRIGGER IF EXISTS audit_trades ON trades;
CREATE TRIGGER audit_trades
    AFTER INSERT OR UPDATE OR DELETE ON trades
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_func();

-- Create admin_actions table for tracking administrative actions
CREATE TABLE IF NOT EXISTS admin_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_user_id UUID NOT NULL REFERENCES users(id),
    action_type TEXT NOT NULL,
    target_type TEXT,
    target_id UUID,
    description TEXT NOT NULL,
    metadata JSONB,
    ip_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_admin_actions_admin_user_id ON admin_actions(admin_user_id);
CREATE INDEX idx_admin_actions_action_type ON admin_actions(action_type);
CREATE INDEX idx_admin_actions_target ON admin_actions(target_type, target_id);
CREATE INDEX idx_admin_actions_created_at ON admin_actions(created_at DESC);

-- Create compliance view for easy reporting
CREATE OR REPLACE VIEW audit_summary AS
SELECT
    table_name,
    action,
    DATE(created_at) as audit_date,
    COUNT(*) as event_count,
    COUNT(DISTINCT user_id) as unique_users,
    COUNT(DISTINCT record_id) as unique_records
FROM audit_logs
GROUP BY table_name, action, DATE(created_at);
