-- Create output commitments table
CREATE TABLE output_commitments (
    commitment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    issuer_user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    title VARCHAR(200) NOT NULL,
    description TEXT NOT NULL,
    required_tokens NUMERIC(20, 8) NOT NULL,
    deadline TIMESTAMPTZ,
    status VARCHAR(30) NOT NULL DEFAULT 'active',
    evidence_url TEXT,
    verified_at TIMESTAMPTZ,
    verified_by UUID REFERENCES users(user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_commitments_issuer ON output_commitments(issuer_user_id);
CREATE INDEX idx_commitments_token ON output_commitments(token_id);
CREATE INDEX idx_commitments_status ON output_commitments(status);
