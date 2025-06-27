-- Drop all Omikuji tables
-- WARNING: This will permanently delete all data in these tables!
-- Run this in your Supabase SQL editor to remove all Omikuji tables

-- Drop indexes first (if they exist)
DROP INDEX IF EXISTS idx_feed_log_feed_name;
DROP INDEX IF EXISTS idx_feed_log_network_name;
DROP INDEX IF EXISTS idx_feed_log_created_at;
DROP INDEX IF EXISTS idx_feed_log_feed_timestamp;
DROP INDEX IF EXISTS idx_feed_log_feed_network;

DROP INDEX IF EXISTS idx_transaction_log_feed_name;
DROP INDEX IF EXISTS idx_transaction_log_network_name;
DROP INDEX IF EXISTS idx_transaction_log_transaction_hash;
DROP INDEX IF EXISTS idx_transaction_log_status;
DROP INDEX IF EXISTS idx_transaction_log_created_at;
DROP INDEX IF EXISTS idx_transaction_log_block_number;
DROP INDEX IF EXISTS idx_transaction_log_feed_network;

DROP INDEX IF EXISTS idx_gas_price_log_network;
DROP INDEX IF EXISTS idx_gas_price_log_created_at;
DROP INDEX IF EXISTS idx_gas_token_prices_symbol;
DROP INDEX IF EXISTS idx_gas_token_prices_updated;

-- Drop tables (CASCADE will drop any dependent objects)
DROP TABLE IF EXISTS feed_log CASCADE;
DROP TABLE IF EXISTS transaction_log CASCADE;
DROP TABLE IF EXISTS gas_price_log CASCADE;
DROP TABLE IF EXISTS gas_token_prices CASCADE;
DROP TABLE IF EXISTS _sqlx_migrations CASCADE;

-- Verify all tables have been dropped
SELECT 'Remaining Omikuji tables:' as message;
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public' 
AND table_name IN ('feed_log', 'transaction_log', 'gas_price_log', 'gas_token_prices', '_sqlx_migrations');

-- Should return no rows if all tables were dropped successfully