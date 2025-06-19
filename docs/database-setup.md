# Database Setup Guide

Omikuji uses PostgreSQL to store historical feed values for analysis and auditing. This guide explains how to set up and configure the database.

## Table of Contents
- [Prerequisites](#prerequisites)
- [PostgreSQL Installation](#postgresql-installation)
- [Database Setup](#database-setup)
- [Configuration](#configuration)
- [Migrations](#migrations)
- [Data Retention](#data-retention)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## Prerequisites

- PostgreSQL 12 or higher
- `psql` command-line tool (usually included with PostgreSQL)
- Database user with CREATE DATABASE privileges

## PostgreSQL Installation

### macOS
```bash
# Using Homebrew
brew install postgresql
brew services start postgresql
```

### Ubuntu/Debian
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql
```

### Docker
```bash
docker run -d \
  --name omikuji-postgres \
  -e POSTGRES_USER=omikuji \
  -e POSTGRES_PASSWORD=your_password \
  -e POSTGRES_DB=omikuji \
  -p 5432:5432 \
  postgres:15
```

## Database Setup

1. **Create a database user** (if not using Docker):
```sql
sudo -u postgres psql
CREATE USER omikuji WITH PASSWORD 'your_secure_password';
```

2. **Create the database**:
```sql
CREATE DATABASE omikuji OWNER omikuji;
GRANT ALL PRIVILEGES ON DATABASE omikuji TO omikuji;
```

3. **Set the DATABASE_URL environment variable**:
```bash
# Add to your .env file
DATABASE_URL=postgres://omikuji:your_secure_password@localhost:5432/omikuji
```

## Configuration

### Environment Variables

The database connection is configured via the `DATABASE_URL` environment variable:

```bash
DATABASE_URL=postgres://username:password@host:port/database
```

Example configurations:

```bash
# Local PostgreSQL
DATABASE_URL=postgres://omikuji:password@localhost:5432/omikuji

# Remote PostgreSQL
DATABASE_URL=postgres://omikuji:password@db.example.com:5432/omikuji

# PostgreSQL with SSL
DATABASE_URL=postgres://omikuji:password@db.example.com:5432/omikuji?sslmode=require
```

### Data Retention Configuration

Configure how long to keep feed data in your `config.yaml`:

```yaml
datafeeds:
  - name: eth_usd
    # ... other config ...
    data_retention_days: 7  # Keep data for 7 days (default)
  
  - name: btc_usd
    # ... other config ...
    data_retention_days: 30 # Keep BTC data for 30 days

# Cleanup task configuration
database_cleanup:
  enabled: true              # Enable automatic cleanup
  schedule: "0 0 * * * *"   # Run every hour at minute 0
```

### Cron Schedule Format

The cleanup schedule uses standard cron format:
```
┌───────────── second (0 - 59)
│ ┌───────────── minute (0 - 59)
│ │ ┌───────────── hour (0 - 23)
│ │ │ ┌───────────── day of month (1 - 31)
│ │ │ │ ┌───────────── month (1 - 12)
│ │ │ │ │ ┌───────────── day of week (0 - 6)
│ │ │ │ │ │
│ │ │ │ │ │
* * * * * *
```

Examples:
- `"0 0 * * * *"` - Every hour at minute 0
- `"0 0 */6 * * *"` - Every 6 hours
- `"0 0 0 * * *"` - Daily at midnight
- `"0 0 3 * * 0"` - Weekly on Sunday at 3 AM

## Migrations

Omikuji automatically runs database migrations on startup. The migrations create:

1. **feed_log table**:
   - `id` - Auto-incrementing primary key
   - `feed_name` - Name of the datafeed
   - `network_name` - Blockchain network name
   - `feed_value` - Retrieved value
   - `feed_timestamp` - Timestamp from the feed
   - `updated_at` - When Omikuji recorded the value
   - `error_status_code` - HTTP error code (if any)
   - `network_error` - Network error flag
   - `created_at` - Record creation timestamp

2. **Indexes** for performance:
   - `idx_feed_log_feed_name`
   - `idx_feed_log_network_name`
   - `idx_feed_log_created_at`
   - `idx_feed_log_feed_timestamp`
   - `idx_feed_log_feed_network`

### Manual Migration

If automatic migration fails, you can run it manually:

```bash
# Check migration files
ls migrations/

# Run manually using psql
psql $DATABASE_URL < migrations/20240101000001_create_feed_log_table.sql
```

## Data Retention

### Automatic Cleanup

The cleanup task runs according to the configured schedule and removes data older than the retention period:

- Each datafeed can have its own retention period
- Default retention is 7 days
- Cleanup runs hourly by default

### Manual Cleanup

To manually clean old data:

```sql
-- Delete data older than 7 days for a specific feed
DELETE FROM feed_log 
WHERE feed_name = 'eth_usd' 
  AND network_name = 'ethereum'
  AND created_at < NOW() - INTERVAL '7 days';

-- Delete all data older than 30 days
DELETE FROM feed_log 
WHERE created_at < NOW() - INTERVAL '30 days';
```

## Monitoring

### Check Database Size

```sql
-- Database size
SELECT pg_database_size('omikuji') / 1024 / 1024 as size_mb;

-- Table sizes
SELECT 
  relname AS table_name,
  pg_size_pretty(pg_total_relation_size(relid)) AS size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC;
```

### Feed Statistics

```sql
-- Feed summary
SELECT 
  feed_name,
  network_name,
  COUNT(*) as log_count,
  MIN(created_at) as oldest_log,
  MAX(created_at) as newest_log,
  COUNT(CASE WHEN error_status_code IS NOT NULL OR network_error = true THEN 1 END) as error_count
FROM feed_log
GROUP BY feed_name, network_name
ORDER BY feed_name, network_name;

-- Recent errors
SELECT 
  feed_name,
  network_name,
  error_status_code,
  network_error,
  created_at
FROM feed_log
WHERE error_status_code IS NOT NULL OR network_error = true
ORDER BY created_at DESC
LIMIT 20;
```

### Performance Monitoring

```sql
-- Slow queries
SELECT 
  query,
  calls,
  total_time,
  mean_time,
  max_time
FROM pg_stat_statements
WHERE query LIKE '%feed_log%'
ORDER BY mean_time DESC
LIMIT 10;
```

## Troubleshooting

### Connection Issues

1. **"DATABASE_URL environment variable not set"**
   - Ensure `.env` file exists and contains `DATABASE_URL`
   - Check that `.env` is loaded (see startup logs)

2. **"Failed to create PostgreSQL connection pool"**
   - Verify PostgreSQL is running: `pg_isready`
   - Check connection string format
   - Verify firewall/network settings

3. **"Failed to run database migrations"**
   - Check database user has CREATE TABLE privileges
   - Verify migrations directory exists
   - Run migrations manually if needed

### Performance Issues

1. **Slow queries**:
   - Check indexes exist: `\d feed_log`
   - Run `VACUUM ANALYZE feed_log;`
   - Consider partitioning for very large datasets

2. **High disk usage**:
   - Reduce retention periods
   - Run cleanup more frequently
   - Consider archiving old data

### Running Without Database

Omikuji can run without a database - it will log a warning and continue:

```
[ERROR] Failed to establish database connection: ...
[ERROR] Continuing without database logging
```

Feed monitoring and contract updates will work normally, but historical data won't be saved.

## Best Practices

1. **Regular Backups**: Set up regular PostgreSQL backups
2. **Monitor Disk Space**: Feed logs can grow quickly with many feeds
3. **Index Maintenance**: Run `VACUUM ANALYZE` periodically
4. **Connection Pooling**: Omikuji uses connection pooling (max 10 connections)
5. **Security**: Use strong passwords and SSL for remote connections