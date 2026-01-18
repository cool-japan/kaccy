-- Create orders table
CREATE TABLE orders (
    order_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    order_type VARCHAR(10) NOT NULL,
    amount NUMERIC(20, 8) NOT NULL,
    price_btc NUMERIC(20, 10) NOT NULL,
    total_btc NUMERIC(20, 10) NOT NULL,
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
