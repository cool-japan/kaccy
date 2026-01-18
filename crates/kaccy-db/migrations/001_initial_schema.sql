-- Kaccy Protocol Initial Schema
-- This migration creates all base tables for the platform

-- Users table
CREATE TABLE IF NOT EXISTS users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    username VARCHAR(50) NOT NULL UNIQUE,
    display_name VARCHAR(100),
    bio TEXT,
    avatar_url VARCHAR(500),
    btc_withdrawal_address VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    kyc_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reputation_score DECIMAL(10, 2) NOT NULL DEFAULT 500.00,
    role VARCHAR(20) NOT NULL DEFAULT 'user'
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_reputation ON users(reputation_score);

-- Tokens (FOT - Future Output Token)
CREATE TABLE IF NOT EXISTS tokens (
    token_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_user_id UUID NOT NULL REFERENCES users(user_id),
    symbol VARCHAR(20) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    total_supply DECIMAL(20, 8) NOT NULL,
    circulating_supply DECIMAL(20, 8) NOT NULL DEFAULT 0,
    initial_price_btc DECIMAL(20, 8) NOT NULL,
    price_increment_btc DECIMAL(20, 8) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'active'
);

CREATE INDEX idx_tokens_issuer ON tokens(issuer_user_id);
CREATE INDEX idx_tokens_symbol ON tokens(symbol);
CREATE INDEX idx_tokens_status ON tokens(status);

-- Balances
CREATE TABLE IF NOT EXISTS balances (
    balance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    amount DECIMAL(20, 8) NOT NULL DEFAULT 0,
    locked_amount DECIMAL(20, 8) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, token_id)
);

CREATE INDEX idx_balances_user ON balances(user_id);
CREATE INDEX idx_balances_token ON balances(token_id);

-- Orders
CREATE TABLE IF NOT EXISTS orders (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    order_type VARCHAR(10) NOT NULL, -- 'buy' or 'sell'
    amount DECIMAL(20, 8) NOT NULL,
    price_btc DECIMAL(20, 8) NOT NULL,
    total_btc DECIMAL(20, 8) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    btc_address VARCHAR(100),
    btc_txid VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_orders_user ON orders(user_id);
CREATE INDEX idx_orders_token ON orders(token_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_btc_address ON orders(btc_address);

-- Trades
CREATE TABLE IF NOT EXISTS trades (
    trade_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    buyer_user_id UUID NOT NULL REFERENCES users(user_id),
    seller_user_id UUID REFERENCES users(user_id), -- NULL for bonding curve buys
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    amount DECIMAL(20, 8) NOT NULL,
    price_btc DECIMAL(20, 8) NOT NULL,
    total_btc DECIMAL(20, 8) NOT NULL,
    platform_fee_btc DECIMAL(20, 8) NOT NULL DEFAULT 0,
    issuer_royalty_btc DECIMAL(20, 8) NOT NULL DEFAULT 0,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trades_buyer ON trades(buyer_user_id);
CREATE INDEX idx_trades_seller ON trades(seller_user_id);
CREATE INDEX idx_trades_token ON trades(token_id);
CREATE INDEX idx_trades_executed_at ON trades(executed_at);

-- Reputation Events
CREATE TABLE IF NOT EXISTS reputation_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    event_type VARCHAR(50) NOT NULL,
    delta DECIMAL(10, 2) NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reputation_events_user ON reputation_events(user_id);
CREATE INDEX idx_reputation_events_created_at ON reputation_events(created_at);

-- Output Commitments
CREATE TABLE IF NOT EXISTS output_commitments (
    commitment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    title VARCHAR(200) NOT NULL,
    description TEXT,
    deadline TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    evidence_url VARCHAR(500),
    evidence_description TEXT,
    verified_at TIMESTAMPTZ,
    verified_by UUID REFERENCES users(user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_commitments_user ON output_commitments(user_id);
CREATE INDEX idx_commitments_token ON output_commitments(token_id);
CREATE INDEX idx_commitments_status ON output_commitments(status);
CREATE INDEX idx_commitments_deadline ON output_commitments(deadline);

-- Issuer Earnings (royalty tracking)
CREATE TABLE IF NOT EXISTS issuer_earnings (
    earning_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    trade_id UUID NOT NULL REFERENCES trades(trade_id),
    amount_btc DECIMAL(20, 8) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_issuer_earnings_user ON issuer_earnings(user_id);
CREATE INDEX idx_issuer_earnings_token ON issuer_earnings(token_id);

-- Pending Payouts (for sell orders)
CREATE TABLE IF NOT EXISTS pending_payouts (
    payout_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    trade_id UUID NOT NULL REFERENCES trades(trade_id),
    amount_btc DECIMAL(20, 8) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    btc_txid VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);

CREATE INDEX idx_pending_payouts_user ON pending_payouts(user_id);
CREATE INDEX idx_pending_payouts_status ON pending_payouts(status);

-- Admin Actions (audit log)
CREATE TABLE IF NOT EXISTS admin_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_user_id UUID NOT NULL REFERENCES users(user_id),
    action_type VARCHAR(50) NOT NULL,
    target_type VARCHAR(50) NOT NULL,
    target_id UUID NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_admin_actions_admin ON admin_actions(admin_user_id);
CREATE INDEX idx_admin_actions_created_at ON admin_actions(created_at);
