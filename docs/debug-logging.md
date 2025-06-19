# Debug Logging Guide

Omikuji uses the Rust `tracing` crate for structured logging. You can control the log level using the `RUST_LOG` environment variable.

## Enabling Debug Logging

To see detailed debug logs including database operations, set the `RUST_LOG` environment variable:

```bash
# Enable debug logging for all omikuji modules
export RUST_LOG=omikuji=debug
omikuji

# Or run with the environment variable inline
RUST_LOG=omikuji=debug omikuji
```

## Database Debug Messages

When debug logging is enabled, you'll see detailed information about database operations:

### Connection Details
- Database connection URL (with credentials masked)
- PostgreSQL version information
- Connection pool configuration

### Feed Log Operations
- Attempts to save feed logs with all parameters
- Successful saves with generated IDs
- Any errors during save operations

### Transaction Log Operations
- Transaction details being saved (gas usage, costs, efficiency)
- Successful saves with generated IDs
- Any errors during save operations

### Example Debug Output

```
2025-06-19T12:34:56.789Z DEBUG omikuji::database::connection: Attempting to connect to database: postgres://***@localhost:5432/omikuji_db
2025-06-19T12:34:56.890Z DEBUG omikuji::database::connection: Connected to database. PostgreSQL version: PostgreSQL 14.5 on x86_64-pc-linux-gnu
2025-06-19T12:34:57.123Z DEBUG omikuji::datafeed::monitor: Saving feed log to database for btc_usd: value=45123.50, timestamp=1719000000
2025-06-19T12:34:57.145Z DEBUG omikuji::database::repository: Successfully saved feed log with id=1234: feed=btc_usd, network=mainnet, value=45123.50
2025-06-19T12:34:58.234Z DEBUG omikuji::contracts::flux_aggregator: Preparing to save transaction log to database for btc_usd on mainnet
2025-06-19T12:34:58.256Z DEBUG omikuji::database::transaction_repository: Successfully saved transaction log with id=567: feed=btc_usd on mainnet
```

## Log Levels

You can use different log levels:

- `error` - Only show errors
- `warn` - Show warnings and errors
- `info` - Show informational messages (default)
- `debug` - Show detailed debug information
- `trace` - Show very detailed trace information

## Module-Specific Logging

You can enable debug logging for specific modules:

```bash
# Debug logging only for database operations
RUST_LOG=omikuji::database=debug omikuji

# Debug logging for datafeed monitoring
RUST_LOG=omikuji::datafeed=debug omikuji

# Multiple modules
RUST_LOG=omikuji::database=debug,omikuji::datafeed=debug omikuji
```

## Combining with Other Tools

You can pipe the output to tools like `jq` for structured log processing:

```bash
RUST_LOG=omikuji=debug omikuji 2>&1 | grep "database" | tail -20
```

## Performance Considerations

Debug logging can be verbose and may impact performance slightly. It's recommended to use `info` level for production deployments and only enable `debug` when troubleshooting issues.