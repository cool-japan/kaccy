-- Create tokens table
CREATE TABLE tokens (
    token_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    issuer_user_id UUID NOT NULL REFERENCES users(user_id),
    symbol VARCHAR(11) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    total_supply NUMERIC(20, 8) NOT NULL,
    circulating_supply NUMERIC(20, 8) NOT NULL DEFAULT 0,
    initial_price_btc NUMERIC(20, 10) NOT NULL,
    price_increment_btc NUMERIC(20, 10) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'active'
);

CREATE INDEX idx_tokens_issuer ON tokens(issuer_user_id);
CREATE INDEX idx_tokens_symbol ON tokens(symbol);
CREATE INDEX idx_tokens_status ON tokens(status);
