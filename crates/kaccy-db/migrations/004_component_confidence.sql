-- Component confidence tracking for reputation scores
CREATE TABLE IF NOT EXISTS component_confidence (
    confidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    component VARCHAR(50) NOT NULL,
    data_points BIGINT NOT NULL DEFAULT 0,
    confidence_level VARCHAR(20) NOT NULL DEFAULT 'VeryLow',
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, component)
);

CREATE INDEX idx_component_confidence_user ON component_confidence(user_id);
CREATE INDEX idx_component_confidence_level ON component_confidence(confidence_level);
CREATE INDEX idx_component_confidence_updated ON component_confidence(last_updated);
