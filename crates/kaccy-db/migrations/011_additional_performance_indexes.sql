-- Additional performance indexes for balances, orders, trades, and audit tables
-- Optimizes frequent query patterns identified in repository analysis

-- ============================================================================
-- BALANCES TABLE INDEXES
-- ============================================================================

-- Composite index for balance lookups (primary access pattern)
-- Used by: BalanceRepository::get_balance(), get_available_balance()
CREATE INDEX IF NOT EXISTS idx_balances_user_token
ON balances(user_id, token_id);

-- Index for token holder queries sorted by balance
-- Used by: BalanceRepository::get_token_holders()
CREATE INDEX IF NOT EXISTS idx_balances_token_amount
ON balances(token_id, amount DESC)
WHERE amount > 0;

-- Covering index for balance queries with common columns
-- Reduces need for table lookups by including frequently accessed columns
CREATE INDEX IF NOT EXISTS idx_balances_user_token_covering
ON balances(user_id, token_id)
INCLUDE (amount, locked_amount, updated_at);

-- ============================================================================
-- ORDERS TABLE INDEXES
-- ============================================================================

-- Composite index for user order queries with status filtering
-- Used by: OrderRepository::get_user_orders() with status filter
CREATE INDEX IF NOT EXISTS idx_orders_user_status
ON orders(user_id, status, created_at DESC);

-- Partial index for pending orders (admin operations)
-- Used by: OrderRepository::get_pending_orders()
CREATE INDEX IF NOT EXISTS idx_orders_pending
ON orders(status, created_at DESC)
WHERE status = 'pending';

-- Index for orders by BTC address
-- Used by: OrderRepository::find_by_btc_address()
CREATE INDEX IF NOT EXISTS idx_orders_btc_address
ON orders(btc_deposit_address)
WHERE btc_deposit_address IS NOT NULL;

-- ============================================================================
-- TRADES TABLE INDEXES
-- ============================================================================

-- Index for buyer trades sorted by time
-- Used by: TradeRepository::get_user_trades() (buyer side)
CREATE INDEX IF NOT EXISTS idx_trades_buyer_executed
ON trades(buyer_user_id, executed_at DESC);

-- Index for seller trades sorted by time
-- Used by: TradeRepository::get_user_trades() (seller side)
CREATE INDEX IF NOT EXISTS idx_trades_seller_executed
ON trades(seller_user_id, executed_at DESC)
WHERE seller_user_id IS NOT NULL;

-- Index for 24h volume calculations
-- Used by: TradeRepository::get_24h_volume()
CREATE INDEX IF NOT EXISTS idx_trades_token_executed
ON trades(token_id, executed_at DESC);

-- ============================================================================
-- TOKENS TABLE INDEXES
-- ============================================================================

-- Composite index for issuer token queries with status
-- Used by: TokenRepository::find_by_issuer() with status filtering
CREATE INDEX IF NOT EXISTS idx_tokens_issuer_status
ON tokens(issuer_user_id, status, created_at DESC);

-- Partial index for active tokens
-- Used by: TokenRepository::list_active()
CREATE INDEX IF NOT EXISTS idx_tokens_active
ON tokens(created_at DESC)
WHERE status = 'active';

-- Index for token symbol lookups (case-insensitive)
-- Used by: TokenRepository::find_by_symbol(), symbol_exists()
CREATE INDEX IF NOT EXISTS idx_tokens_symbol_lower
ON tokens(LOWER(symbol));

-- ============================================================================
-- AUDIT LOGS TABLE INDEXES
-- ============================================================================

-- Composite index for user audit log queries
-- Used by: AuditRepository::get_user_audit_logs()
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created
ON audit_logs(user_id, created_at DESC);

-- Index for audit logs by action type
-- Used by: AuditRepository::get_logs_by_action()
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created
ON audit_logs(action, created_at DESC);

-- Index for audit logs by table name (schema change tracking)
-- Used by: AuditRepository::get_table_changes()
CREATE INDEX IF NOT EXISTS idx_audit_logs_table_name
ON audit_logs(table_name, created_at DESC)
WHERE table_name IS NOT NULL;

-- ============================================================================
-- API KEYS TABLE INDEXES
-- ============================================================================

-- Partial index for active API keys (already exists in 009_api_keys.sql)
-- Verify: idx_api_keys_user_active

-- Index for API key hash lookups (authentication)
-- Used by: ApiKeyRepository::find_by_hash()
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash
ON api_keys(key_hash);

-- ============================================================================
-- API KEY USAGE TABLE INDEXES
-- ============================================================================

-- Composite index for usage queries by key and time
-- Used by: ApiKeyUsageRepository::get_stats(), get_by_date_range()
CREATE INDEX IF NOT EXISTS idx_api_key_usage_key_created
ON api_key_usage(api_key_id, created_at DESC);

-- Index for hourly usage aggregation
-- Used by: ApiKeyUsageRepository::get_hourly_usage()
CREATE INDEX IF NOT EXISTS idx_api_key_usage_key_hour
ON api_key_usage(api_key_id, created_at)
WHERE created_at >= NOW() - INTERVAL '24 hours';

-- Index for endpoint statistics
-- Used by: ApiKeyUsageRepository::get_endpoint_stats()
CREATE INDEX IF NOT EXISTS idx_api_key_usage_endpoint
ON api_key_usage(api_key_id, endpoint, method, status_code);

-- ============================================================================
-- INDEX COMMENTS FOR MAINTENANCE
-- ============================================================================

COMMENT ON INDEX idx_balances_user_token IS 'Primary balance lookup pattern';
COMMENT ON INDEX idx_balances_token_amount IS 'Token holder ranking by balance';
COMMENT ON INDEX idx_orders_user_status IS 'User order history with status filter';
COMMENT ON INDEX idx_orders_pending IS 'Partial index for pending order admin queries';
COMMENT ON INDEX idx_trades_buyer_executed IS 'Buyer trade history optimized for time-series';
COMMENT ON INDEX idx_trades_seller_executed IS 'Seller trade history optimized for time-series';
COMMENT ON INDEX idx_tokens_issuer_status IS 'Issuer token portfolio with status filtering';
COMMENT ON INDEX idx_audit_logs_user_created IS 'User audit trail queries';
COMMENT ON INDEX idx_api_key_usage_key_created IS 'API key usage analytics and rate limiting';
