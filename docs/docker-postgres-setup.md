# Docker PostgreSQL Setup for Omikuji

This guide explains how to quickly set up a PostgreSQL database for Omikuji using Docker Compose.

## Quick Start

1. **Start PostgreSQL**:
   ```bash
   docker-compose up -d postgres
   ```

2. **Set environment variable**:
   ```bash
   export DATABASE_URL="postgresql://omikuji:omikuji_password@localhost:5433/omikuji_db"
   ```

3. **Run Omikuji** with your config file:
   ```bash
   cargo run -- --config config.yaml
   ```

## Docker Compose Services

### PostgreSQL Database

- **Image**: PostgreSQL 16 Alpine (lightweight)
- **Port**: 5433 (mapped from container's 5432)
- **Database**: omikuji_db
- **Username**: omikuji
- **Password**: omikuji_password
- **Data persistence**: Stored in Docker volume
- **Health checks**: Automatic container health monitoring
- **Resource limits**: 2GB memory limit, 1GB reserved

### pgAdmin (Optional)

A web-based PostgreSQL administration tool:

```bash
# Start with pgAdmin
docker-compose --profile tools up -d

# Access pgAdmin at http://localhost:5050
# Login: admin@example.com / admin_password
```

## Common Commands

### Start Services
```bash
# Start PostgreSQL only
docker-compose up -d postgres

# Start all services (including pgAdmin)
docker-compose --profile tools up -d

# View logs
docker-compose logs -f postgres
```

### Stop Services
```bash
# Stop containers (data persists)
docker-compose down

# Stop and remove all data (WARNING: deletes database!)
docker-compose down -v
```

### Database Operations
```bash
# Connect to PostgreSQL
docker-compose exec postgres psql -U omikuji -d omikuji_db

# Run migrations manually
export DATABASE_URL="postgresql://omikuji:omikuji_password@localhost:5433/omikuji_db"
sqlx migrate run

# Check feed statistics
docker-compose exec postgres psql -U omikuji -d omikuji_db -c "SELECT * FROM omikuji.feed_stats;"
```

## Performance Optimization

The setup includes automatic PostgreSQL optimization for time-series data:

- Tuned memory settings for better performance
- Indexes on commonly queried columns
- Query logging for slow queries (> 1 second)
- Parallel query execution enabled
- Checkpoint settings optimized for write-heavy workloads

## Monitoring

### View Feed Statistics
```sql
-- Connect to database
docker-compose exec postgres psql -U omikuji -d omikuji_db

-- View feed summary
SELECT * FROM omikuji.feed_stats;

-- Recent feed values
SELECT feed_name, network_name, feed_value, created_at 
FROM feed_log 
ORDER BY created_at DESC 
LIMIT 20;

-- Error rate by feed
SELECT 
    feed_name,
    COUNT(*) as total,
    SUM(CASE WHEN error_status_code IS NOT NULL THEN 1 ELSE 0 END) as errors,
    ROUND(100.0 * SUM(CASE WHEN error_status_code IS NOT NULL THEN 1 ELSE 0 END) / COUNT(*), 2) as error_rate
FROM feed_log
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY feed_name;
```

### Container Resource Usage
```bash
# View resource usage
docker stats omikuji-postgres

# Check disk usage
docker system df
```

## Backup and Restore

### Backup Database
```bash
# Create backup
docker-compose exec postgres pg_dump -U omikuji omikuji_db > backup.sql

# Create compressed backup with timestamp
docker-compose exec postgres pg_dump -U omikuji omikuji_db | gzip > backup_$(date +%Y%m%d_%H%M%S).sql.gz
```

### Restore Database
```bash
# Restore from backup
docker-compose exec -T postgres psql -U omikuji omikuji_db < backup.sql

# Restore from compressed backup
gunzip -c backup_20240119_120000.sql.gz | docker-compose exec -T postgres psql -U omikuji omikuji_db
```

## Production Considerations

For production deployments:

1. **Change default passwords** in docker-compose.yaml
2. **Use external volumes** for data persistence
3. **Set up regular backups** with a cron job
4. **Monitor disk space** as feed logs can grow large
5. **Consider using** a managed PostgreSQL service for better reliability
6. **Enable SSL/TLS** for database connections
7. **Implement** connection pooling if running multiple Omikuji instances

## Troubleshooting

### Container won't start
```bash
# Check logs
docker-compose logs postgres

# Remove and recreate (WARNING: deletes data)
docker-compose down -v
docker-compose up -d postgres
```

### Connection refused
- Ensure PostgreSQL is running: `docker-compose ps`
- Check if port 5433 is already in use: `lsof -i :5433`
- Verify DATABASE_URL is correct (note: using port 5433, not 5432)

### Disk space issues
```bash
# Check volume size
docker system df

# Clean up old data manually
docker-compose exec postgres psql -U omikuji -d omikuji_db -c "DELETE FROM feed_log WHERE created_at < NOW() - INTERVAL '30 days';"

# Reclaim space
docker-compose exec postgres psql -U omikuji -d omikuji_db -c "VACUUM FULL feed_log;"
```