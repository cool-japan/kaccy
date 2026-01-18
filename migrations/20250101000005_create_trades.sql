-- Create trades table
CREATE TABLE trades (
    trade_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    buyer_user_id UUID NOT NULL REFERENCES users(user_id),
    seller_user_id UUID REFERENCES users(user_id),
    token_id UUID NOT NULL REFERENCES tokens(token_id),
    amount NUMERIC(20, 8) NOT NULL,
    price_btc NUMERIC(20, 10) NOT NULL,
    total_btc NUMERIC(20, 10) NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trades_buyer ON trades(buyer_user_id);
CREATE INDEX idx_trades_seller ON trades(seller_user_id);
CREATE INDEX idx_trades_token ON trades(token_id);
CREATE INDEX idx_trades_executed_at ON trades(executed_at);
