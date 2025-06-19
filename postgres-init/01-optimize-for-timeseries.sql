-- Optimize PostgreSQL for time-series data storage
-- This script runs automatically when the container is first created

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- Optimize shared buffers and work memory for time-series workloads
ALTER SYSTEM SET shared_buffers = '256MB';
ALTER SYSTEM SET effective_cache_size = '1GB';
ALTER SYSTEM SET work_mem = '16MB';
ALTER SYSTEM SET maintenance_work_mem = '64MB';

-- Optimize checkpoint settings for write-heavy workloads
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
ALTER SYSTEM SET wal_buffers = '16MB';
ALTER SYSTEM SET max_wal_size = '1GB';
ALTER SYSTEM SET min_wal_size = '80MB';

-- Enable parallel queries for better read performance
ALTER SYSTEM SET max_parallel_workers_per_gather = 2;
ALTER SYSTEM SET max_parallel_workers = 4;

-- Optimize for SSD storage (if applicable)
ALTER SYSTEM SET random_page_cost = 1.1;
ALTER SYSTEM SET effective_io_concurrency = 200;

-- Enable query performance insights
ALTER SYSTEM SET log_min_duration_statement = 1000; -- Log queries slower than 1 second
ALTER SYSTEM SET log_line_prefix = '%t [%p]: [%l-1] user=%u,db=%d,app=%a,client=%h ';

-- Create a dedicated schema for Omikuji
CREATE SCHEMA IF NOT EXISTS omikuji;

-- Grant privileges to the omikuji user
GRANT ALL PRIVILEGES ON SCHEMA omikuji TO omikuji;
GRANT ALL PRIVILEGES ON DATABASE omikuji_db TO omikuji;

-- Create helpful views for monitoring
CREATE OR REPLACE VIEW omikuji.feed_stats AS
SELECT 
    feed_name,
    network_name,
    COUNT(*) as total_logs,
    COUNT(CASE WHEN error_status_code IS NOT NULL OR network_error = true THEN 1 END) as error_count,
    MIN(created_at) as oldest_log,
    MAX(created_at) as newest_log,
    AVG(feed_value) FILTER (WHERE error_status_code IS NULL AND network_error = false) as avg_value,
    MIN(feed_value) FILTER (WHERE error_status_code IS NULL AND network_error = false) as min_value,
    MAX(feed_value) FILTER (WHERE error_status_code IS NULL AND network_error = false) as max_value
FROM feed_log
GROUP BY feed_name, network_name;

-- Create an index on created_at for efficient cleanup queries
CREATE INDEX IF NOT EXISTS idx_feed_log_created_at ON feed_log(created_at);

-- Create a composite index for common query patterns
CREATE INDEX IF NOT EXISTS idx_feed_log_feed_network_created 
ON feed_log(feed_name, network_name, created_at DESC);

-- Add table partitioning comment (for future implementation)
COMMENT ON TABLE feed_log IS 'Main table for storing feed values. Consider partitioning by created_at for datasets > 100M rows';

-- Reload configuration
SELECT pg_reload_conf();