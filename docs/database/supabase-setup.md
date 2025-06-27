# Supabase Setup Guide for Omikuji

This guide helps you set up Omikuji with Supabase as your database backend.

## Quick Setup

### 1. Create Tables in Supabase

1. Open your Supabase project dashboard
2. Go to **SQL Editor**
3. Copy the contents of `supabase_setup.sql` 
4. Paste and run it in the SQL editor

### 2. Configure Database Connection

Set your DATABASE_URL environment variable:

```bash
# Format for Supabase pooled connection (recommended)
export DATABASE_URL="postgresql://postgres.[project-ref]:[password]@[region].pooler.supabase.com:5432/postgres?sslmode=require"

# Or use direct connection (limited concurrent connections)
export DATABASE_URL="postgresql://postgres:[password]@db.[project-ref].supabase.co:5432/postgres"
```

### 3. Skip Migrations

Since you've already created the tables manually, set:

```bash
export SKIP_MIGRATIONS=true
```

### 4. Run Omikuji

```bash
omikuji
```

## Troubleshooting

### Connection Issues

1. **Password placeholder**: Make sure to replace `[password]` with your actual database password from Supabase dashboard

2. **SSL Required**: Supabase requires SSL. Add `?sslmode=require` to your connection string

3. **Test connection**:
   ```bash
   psql "$DATABASE_URL" -c "SELECT 1;"
   ```

### Permission Issues

If you get permission errors, run these in Supabase SQL editor:

```sql
-- Grant full permissions to postgres user
GRANT ALL ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO postgres;
GRANT USAGE ON SCHEMA public TO postgres;
```

### Verify Tables

To verify tables were created correctly:

```sql
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public' 
AND table_name IN ('feed_log', 'transaction_log', 'gas_price_log', 'gas_token_prices');
```

### Row Level Security (RLS)

By default, the setup script doesn't enable RLS. If you want to enable it:

1. Enable RLS on tables:
   ```sql
   ALTER TABLE feed_log ENABLE ROW LEVEL SECURITY;
   ALTER TABLE transaction_log ENABLE ROW LEVEL SECURITY;
   ```

2. Create policies (example for service role):
   ```sql
   CREATE POLICY "Service role full access" ON feed_log 
   FOR ALL TO service_role USING (true);
   ```

## What Gets Created

- `feed_log` - Stores historical feed values
- `transaction_log` - Tracks blockchain transactions
- `gas_price_log` - Records gas prices over time
- `gas_token_prices` - Caches token prices for gas calculations
- `_sqlx_migrations` - SQLx migration tracking (for compatibility)

## Environment Variables

```bash
# Required
export DATABASE_URL="your-supabase-connection-string"

# Optional
export SKIP_MIGRATIONS=true  # Skip automatic migrations
```

## Using Different Connection Types

### Pooled Connection (Recommended)
- URL format: `[region].pooler.supabase.com`
- Better for serverless/high-concurrency
- Transaction mode pooling
- May have some limitations with prepared statements

### Direct Connection
- URL format: `db.[project-ref].supabase.co`
- Session mode (full PostgreSQL features)
- Limited concurrent connections
- Better for long-running operations

## Next Steps

1. Monitor your database usage in Supabase dashboard
2. Set up database backups if needed
3. Configure alerts for high usage
4. Consider implementing data retention policies for historical data