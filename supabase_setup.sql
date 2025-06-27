-- Supabase setup script for Omikuji
-- Run this in your Supabase SQL editor to create all required tables

-- Create feed_log table for storing historical feed values
CREATE TABLE IF NOT EXISTS feed_log (
    id SERIAL PRIMARY KEY,
    feed_name VARCHAR(255) NOT NULL,
    network_name VARCHAR(255) NOT NULL,
    feed_value DOUBLE PRECISION NOT NULL,
    feed_timestamp BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    error_status_code INTEGER,
    network_error BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_feed_log_feed_name ON feed_log(feed_name);
CREATE INDEX IF NOT EXISTS idx_feed_log_network_name ON feed_log(network_name);
CREATE INDEX IF NOT EXISTS idx_feed_log_created_at ON feed_log(created_at);
CREATE INDEX IF NOT EXISTS idx_feed_log_feed_timestamp ON feed_log(feed_timestamp);
CREATE INDEX IF NOT EXISTS idx_feed_log_feed_network ON feed_log(feed_name, network_name);

-- Add comments to the table
COMMENT ON TABLE feed_log IS 'Historical log of all feed values retrieved by Omikuji';
COMMENT ON COLUMN feed_log.id IS 'Auto-incrementing internal feed ID';
COMMENT ON COLUMN feed_log.feed_name IS 'Feed name as defined in config.yaml';
COMMENT ON COLUMN feed_log.network_name IS 'Network name for the feed';
COMMENT ON COLUMN feed_log.feed_value IS 'The value retrieved from the feed';
COMMENT ON COLUMN feed_log.feed_timestamp IS 'Timestamp as reported by the feed (Unix timestamp)';
COMMENT ON COLUMN feed_log.updated_at IS 'Timestamp when the system recorded the value';
COMMENT ON COLUMN feed_log.error_status_code IS 'HTTP status code if different from 200';
COMMENT ON COLUMN feed_log.network_error IS 'Whether there was a network error (no HTTP response)';
COMMENT ON COLUMN feed_log.created_at IS 'Timestamp when the record was created';

-- Create transaction_log table for tracking blockchain transactions
CREATE TABLE IF NOT EXISTS transaction_log (
    id SERIAL PRIMARY KEY,
    feed_name VARCHAR(255) NOT NULL,
    network_name VARCHAR(255) NOT NULL,
    transaction_hash VARCHAR(66) NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42) NOT NULL,
    value NUMERIC(78, 0) NOT NULL DEFAULT 0,
    gas_price BIGINT,
    gas_limit BIGINT,
    gas_used BIGINT,
    block_number BIGINT,
    block_hash VARCHAR(66),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    error_message TEXT,
    feed_value DOUBLE PRECISION,
    feed_timestamp BIGINT,
    deviation_percentage DOUBLE PRECISION,
    update_reason VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_transaction_log_feed_name ON transaction_log(feed_name);
CREATE INDEX IF NOT EXISTS idx_transaction_log_network_name ON transaction_log(network_name);
CREATE INDEX IF NOT EXISTS idx_transaction_log_transaction_hash ON transaction_log(transaction_hash);
CREATE INDEX IF NOT EXISTS idx_transaction_log_status ON transaction_log(status);
CREATE INDEX IF NOT EXISTS idx_transaction_log_created_at ON transaction_log(created_at);
CREATE INDEX IF NOT EXISTS idx_transaction_log_block_number ON transaction_log(block_number);
CREATE INDEX IF NOT EXISTS idx_transaction_log_feed_network ON transaction_log(feed_name, network_name);

-- Add comments to the table
COMMENT ON TABLE transaction_log IS 'Log of all blockchain transactions sent by Omikuji';
COMMENT ON COLUMN transaction_log.id IS 'Auto-incrementing internal transaction ID';
COMMENT ON COLUMN transaction_log.feed_name IS 'Feed name that triggered this transaction';
COMMENT ON COLUMN transaction_log.network_name IS 'Blockchain network name';
COMMENT ON COLUMN transaction_log.transaction_hash IS 'Blockchain transaction hash';
COMMENT ON COLUMN transaction_log.from_address IS 'Sender address (wallet)';
COMMENT ON COLUMN transaction_log.to_address IS 'Contract address';
COMMENT ON COLUMN transaction_log.value IS 'ETH/native token value sent (in wei)';
COMMENT ON COLUMN transaction_log.gas_price IS 'Gas price in wei';
COMMENT ON COLUMN transaction_log.gas_limit IS 'Gas limit for the transaction';
COMMENT ON COLUMN transaction_log.gas_used IS 'Actual gas used (filled after confirmation)';
COMMENT ON COLUMN transaction_log.block_number IS 'Block number when transaction was mined';
COMMENT ON COLUMN transaction_log.block_hash IS 'Block hash when transaction was mined';
COMMENT ON COLUMN transaction_log.status IS 'Transaction status: pending, confirmed, failed';
COMMENT ON COLUMN transaction_log.error_message IS 'Error message if transaction failed';
COMMENT ON COLUMN transaction_log.feed_value IS 'The feed value that triggered this update';
COMMENT ON COLUMN transaction_log.feed_timestamp IS 'Feed timestamp for the value';
COMMENT ON COLUMN transaction_log.deviation_percentage IS 'Percentage deviation that triggered update';
COMMENT ON COLUMN transaction_log.update_reason IS 'Reason for update: deviation, time, force';
COMMENT ON COLUMN transaction_log.created_at IS 'When the transaction was created';
COMMENT ON COLUMN transaction_log.confirmed_at IS 'When the transaction was confirmed on-chain';
COMMENT ON COLUMN transaction_log.updated_at IS 'Last update to this record';

-- Create gas price tracking tables
CREATE TABLE IF NOT EXISTS gas_price_log (
    id SERIAL PRIMARY KEY,
    network VARCHAR(100) NOT NULL,
    block_number BIGINT,
    base_fee_gwei DOUBLE PRECISION,
    priority_fee_gwei DOUBLE PRECISION,
    total_fee_gwei DOUBLE PRECISION,
    gas_token VARCHAR(10) NOT NULL DEFAULT 'ETH',
    gas_token_price_usd DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS gas_token_prices (
    id SERIAL PRIMARY KEY,
    token_symbol VARCHAR(10) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    price_usd DOUBLE PRECISION NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source VARCHAR(50) NOT NULL DEFAULT 'coingecko',
    UNIQUE(token_symbol)
);

-- Create indexes for gas price tables
CREATE INDEX IF NOT EXISTS idx_gas_price_log_network ON gas_price_log(network);
CREATE INDEX IF NOT EXISTS idx_gas_price_log_created_at ON gas_price_log(created_at);
CREATE INDEX IF NOT EXISTS idx_gas_token_prices_symbol ON gas_token_prices(token_symbol);
CREATE INDEX IF NOT EXISTS idx_gas_token_prices_updated ON gas_token_prices(last_updated);

-- Create SQLx migrations table (if using SKIP_MIGRATIONS=true)
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);

-- Grant permissions (adjust based on your Supabase user)
-- Note: You may need to adjust these based on your specific Supabase setup
GRANT ALL ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO postgres;

-- Enable Row Level Security (optional but recommended for Supabase)
-- Uncomment these lines if you want to enable RLS
-- ALTER TABLE feed_log ENABLE ROW LEVEL SECURITY;
-- ALTER TABLE transaction_log ENABLE ROW LEVEL SECURITY;
-- ALTER TABLE gas_price_log ENABLE ROW LEVEL SECURITY;
-- ALTER TABLE gas_token_prices ENABLE ROW LEVEL SECURITY;

-- Create policies for RLS (if enabled)
-- Example: Allow all operations for authenticated users
-- CREATE POLICY "Allow all for authenticated" ON feed_log FOR ALL TO authenticated USING (true);
-- CREATE POLICY "Allow all for authenticated" ON transaction_log FOR ALL TO authenticated USING (true);
-- CREATE POLICY "Allow all for authenticated" ON gas_price_log FOR ALL TO authenticated USING (true);
-- CREATE POLICY "Allow all for authenticated" ON gas_token_prices FOR ALL TO authenticated USING (true);

-- Verify tables were created
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public' 
AND table_name IN ('feed_log', 'transaction_log', 'gas_price_log', 'gas_token_prices', '_sqlx_migrations')
ORDER BY table_name;