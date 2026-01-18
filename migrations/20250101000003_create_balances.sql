-- Create balances table
CREATE TABLE balances (
    balance_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    amount NUMERIC(20, 8) NOT NULL DEFAULT 0,
    locked_amount NUMERIC(20, 8) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, token_id)
);

CREATE INDEX idx_balances_user ON balances(user_id);
CREATE INDEX idx_balances_token ON balances(token_id);
