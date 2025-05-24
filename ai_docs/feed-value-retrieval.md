# Feed Value Retrieval Design

## Overview

This document describes the design and implementation of the feed value retrieval feature for Omikuji. The system fetches price data from external HTTP APIs, extracts values using JSON path notation, and logs the results.

## Core Components

### 1. HTTP Fetcher (`datafeed/fetcher.rs`)
- Uses `reqwest` client to make HTTP requests
- Sends `Accept: application/json` header with all requests
- Handles HTTP status codes (only 200 is considered success)
- Returns raw JSON response for processing

### 2. JSON Path Extractor (`datafeed/json_extractor.rs`)
- Parses dot-notation paths (e.g., "RAW.ETH.USD.PRICE")
- Traverses JSON structure following the path components
- Extracts values as floats
- Handles optional timestamp extraction
- Generates current timestamp if timestamp path not specified

### 3. Feed Monitor (`datafeed/monitor.rs`)
- Manages a single datafeed's polling cycle
- Runs in its own tokio task for concurrent execution
- Performs the following in a loop:
  1. Fetch data from feed URL
  2. Parse JSON response
  3. Extract value and timestamp
  4. Log results
  5. Sleep for `check_frequency` seconds
- Continues on errors (logs them but doesn't stop)

### 4. Feed Manager (`datafeed/manager.rs`)
- Coordinates all feed monitors
- Spawns a separate tokio task for each datafeed
- Provides start/stop functionality
- Handles graceful shutdown

## Architecture

```
┌─────────────┐
│   main.rs   │
└──────┬──────┘
       │ starts
       ▼
┌─────────────────┐
│  FeedManager    │
└────────┬────────┘
         │ spawns tasks
         ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  FeedMonitor    │ │  FeedMonitor    │ │  FeedMonitor    │
│  (ETH/USD)      │ │  (BTC/USD)      │ │  (...)          │
└─────────────────┘ └─────────────────┘ └─────────────────┘
         │                   │                   │
         ▼                   ▼                   ▼
    HTTP Fetch          HTTP Fetch          HTTP Fetch
         │                   │                   │
         ▼                   ▼                   ▼
   JSON Extract        JSON Extract        JSON Extract
         │                   │                   │
         ▼                   ▼                   ▼
    Log Results         Log Results         Log Results
```

## Key Implementation Details

### Error Handling
- HTTP errors are logged but don't stop the monitor
- JSON parsing errors are logged with context
- Path extraction errors include the failing path component
- All errors continue to the next polling cycle

### Concurrency
- Each datafeed runs independently in its own tokio task
- No shared state between monitors (logging only)
- Uses tokio::time::interval for accurate scheduling

### JSON Path Processing
Example: "RAW.ETH.USD.PRICE"
1. Split by '.' → ["RAW", "ETH", "USD", "PRICE"]
2. Navigate JSON: json["RAW"]["ETH"]["USD"]["PRICE"]
3. Extract final value as f64

### Logging Format
```
[INFO] Datafeed eth_usd: value=2045.34, timestamp=1748068861
[ERROR] Datafeed eth_usd: HTTP error: connection timeout
[ERROR] Datafeed eth_usd: JSON path error at 'USD': key not found
```

## Module Structure

```
src/datafeed/
├── mod.rs           # Module exports
├── fetcher.rs       # HTTP client implementation
├── json_extractor.rs # JSON path extraction logic
├── monitor.rs       # Individual feed monitor
├── manager.rs       # Feed manager coordinator
└── tests.rs         # Unit tests
```

## Testing Strategy

1. **Unit Tests**
   - JSON path extraction with various data structures
   - Error cases (missing keys, wrong types)
   - Timestamp generation fallback

2. **Integration Tests**
   - Mock HTTP server for testing fetcher
   - Full monitoring cycle with test data

3. **Manual Testing**
   - Real API endpoints (CryptoCompare)
   - Multiple concurrent feeds
   - Error recovery scenarios