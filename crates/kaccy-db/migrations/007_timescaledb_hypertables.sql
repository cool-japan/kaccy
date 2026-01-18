-- Add TimescaleDB extension (optional - only if TimescaleDB is installed)
-- This migration is safe to run even if TimescaleDB is not available
-- The extension creation will fail silently if not installed

-- Attempt to create TimescaleDB extension (may fail if not installed)
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'TimescaleDB extension not available. Continuing with regular tables.';
END
$$;

-- Price History Table
-- Stores historical price data for all tokens
CREATE TABLE IF NOT EXISTS price_history (
    time TIMESTAMPTZ NOT NULL,
    token_id UUID NOT NULL,
    price_satoshis BIGINT NOT NULL,
    supply BIGINT NOT NULL,
    market_cap_satoshis BIGINT NOT NULL,
    CONSTRAINT price_history_pkey PRIMARY KEY (time, token_id)
);

-- Volume History Table
-- Stores historical trading volume data for all tokens
CREATE TABLE IF NOT EXISTS volume_history (
    time TIMESTAMPTZ NOT NULL,
    token_id UUID NOT NULL,
    buy_volume_satoshis BIGINT NOT NULL DEFAULT 0,
    sell_volume_satoshis BIGINT NOT NULL DEFAULT 0,
    trade_count INTEGER NOT NULL DEFAULT 0,
    unique_traders INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT volume_history_pkey PRIMARY KEY (time, token_id)
);

-- Platform-wide Volume History
-- Aggregated volume metrics for the entire platform
CREATE TABLE IF NOT EXISTS platform_volume_history (
    time TIMESTAMPTZ NOT NULL PRIMARY KEY,
    total_volume_satoshis BIGINT NOT NULL DEFAULT 0,
    trade_count INTEGER NOT NULL DEFAULT 0,
    active_tokens INTEGER NOT NULL DEFAULT 0,
    active_traders INTEGER NOT NULL DEFAULT 0,
    fees_collected_satoshis BIGINT NOT NULL DEFAULT 0
);

-- Try to convert tables to hypertables if TimescaleDB is available
-- This will fail silently if TimescaleDB is not installed
DO $$
BEGIN
    -- Convert price_history to hypertable with 1-day chunks
    PERFORM create_hypertable('price_history', 'time',
        chunk_time_interval => INTERVAL '1 day',
        if_not_exists => TRUE
    );

    -- Convert volume_history to hypertable with 1-day chunks
    PERFORM create_hypertable('volume_history', 'time',
        chunk_time_interval => INTERVAL '1 day',
        if_not_exists => TRUE
    );

    -- Convert platform_volume_history to hypertable with 1-day chunks
    PERFORM create_hypertable('platform_volume_history', 'time',
        chunk_time_interval => INTERVAL '1 day',
        if_not_exists => TRUE
    );

    RAISE NOTICE 'Successfully created TimescaleDB hypertables';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Could not create hypertables (TimescaleDB may not be installed). Tables created as regular tables.';
END
$$;

-- Indexes for efficient querying (work with or without TimescaleDB)
CREATE INDEX IF NOT EXISTS idx_price_history_token_time
    ON price_history (token_id, time DESC);

CREATE INDEX IF NOT EXISTS idx_volume_history_token_time
    ON volume_history (token_id, time DESC);

-- Compression policy (only works if TimescaleDB is available)
DO $$
BEGIN
    -- Enable compression on price_history
    ALTER TABLE price_history SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'token_id'
    );

    -- Add compression policy: compress chunks older than 7 days
    PERFORM add_compression_policy('price_history', INTERVAL '7 days');

    -- Enable compression on volume_history
    ALTER TABLE volume_history SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'token_id'
    );

    PERFORM add_compression_policy('volume_history', INTERVAL '7 days');

    -- Enable compression on platform_volume_history
    ALTER TABLE platform_volume_history SET (
        timescaledb.compress
    );

    PERFORM add_compression_policy('platform_volume_history', INTERVAL '7 days');

    RAISE NOTICE 'Successfully configured compression policies';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Could not configure compression (TimescaleDB may not be installed)';
END
$$;

-- Retention policy (only works if TimescaleDB is available)
DO $$
BEGIN
    -- Keep price history for 2 years
    PERFORM add_retention_policy('price_history', INTERVAL '2 years');

    -- Keep volume history for 2 years
    PERFORM add_retention_policy('volume_history', INTERVAL '2 years');

    -- Keep platform volume history for 5 years
    PERFORM add_retention_policy('platform_volume_history', INTERVAL '5 years');

    RAISE NOTICE 'Successfully configured retention policies';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Could not configure retention policies (TimescaleDB may not be installed)';
END
$$;

-- Continuous aggregates for common queries (only works if TimescaleDB is available)
DO $$
BEGIN
    -- Hourly price aggregates
    CREATE MATERIALIZED VIEW IF NOT EXISTS price_history_hourly
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 hour', time) AS bucket,
        token_id,
        first(price_satoshis, time) AS open_price,
        max(price_satoshis) AS high_price,
        min(price_satoshis) AS low_price,
        last(price_satoshis, time) AS close_price,
        last(supply, time) AS final_supply,
        last(market_cap_satoshis, time) AS final_market_cap
    FROM price_history
    GROUP BY bucket, token_id;

    -- Add refresh policy: refresh every hour, covering last 2 hours
    PERFORM add_continuous_aggregate_policy('price_history_hourly',
        start_offset => INTERVAL '2 hours',
        end_offset => INTERVAL '1 minute',
        schedule_interval => INTERVAL '1 hour'
    );

    -- Daily volume aggregates
    CREATE MATERIALIZED VIEW IF NOT EXISTS volume_history_daily
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 day', time) AS bucket,
        token_id,
        sum(buy_volume_satoshis) AS total_buy_volume,
        sum(sell_volume_satoshis) AS total_sell_volume,
        sum(trade_count) AS total_trades,
        max(unique_traders) AS max_unique_traders
    FROM volume_history
    GROUP BY bucket, token_id;

    PERFORM add_continuous_aggregate_policy('volume_history_daily',
        start_offset => INTERVAL '3 days',
        end_offset => INTERVAL '1 hour',
        schedule_interval => INTERVAL '1 day'
    );

    RAISE NOTICE 'Successfully created continuous aggregates';
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'Could not create continuous aggregates (TimescaleDB may not be installed)';
END
$$;
