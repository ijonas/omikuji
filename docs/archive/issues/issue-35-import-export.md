# Issue #35: CLI Import/Export Functionality

## Status
- **Status**: OPEN
- **Type**: Enhancement
- **Parent Issue**: #32
- **Labels**: enhancement, feature, cli

## Overview
Add import/export capabilities to allow configuration sharing, format conversion, and integration with other tools.

## Proposed Features

### 1. Multi-Format Export
Export configurations to various formats:

```bash
# Export to JSON
omikuji config export --format json > config.json

# Export to TOML
omikuji config export --format toml > config.toml

# Export specific sections
omikuji config export --format json --only feeds
omikuji config export --format csv --only feeds > feeds.csv

# Export with filtering
omikuji config export --network ethereum --format json
```

**Supported Formats:**
- JSON (full fidelity)
- TOML (full fidelity)
- CSV (feeds only, tabular format)
- ENV (environment variables format)
- Markdown (human-readable documentation)

### 2. Multi-Format Import
Import configurations from various sources:

```bash
# Import from JSON
omikuji config import config.json

# Import from TOML
omikuji config import config.toml

# Import feeds from CSV
omikuji feed import feeds.csv

# Import with merge strategies
omikuji config import new-feeds.json --merge
omikuji config import new-feeds.json --replace
```

### 3. CSV Feed Management
Bulk feed operations via CSV:

```bash
# Export feeds to CSV
omikuji feed export > feeds.csv

# Import feeds from CSV
omikuji feed import feeds.csv

# Update feeds from CSV
omikuji feed update --from-csv updates.csv
```

**CSV Format:**
```csv
name,network,contract_address,contract_type,check_frequency,deviation_threshold,feed_url,json_path
eth_usd,ethereum,0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419,fluxmon,60,0.5,https://api.example.com/eth,data.price
btc_usd,ethereum,0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c,fluxmon,60,1.0,https://api.example.com/btc,data.price
```

### 4. Environment Variable Export
Generate environment variable configurations:

```bash
# Export as env vars
omikuji config export --format env

# Output:
# OMIKUJI_NETWORK_ETHEREUM_RPC_URL=https://eth.llamarpc.com
# OMIKUJI_FEED_ETH_USD_CONTRACT=0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419
# OMIKUJI_FEED_ETH_USD_DEVIATION=0.5
```

### 5. Documentation Generation
Generate human-readable documentation:

```bash
# Generate markdown documentation
omikuji config export --format markdown > CONFIG.md

# Include examples and descriptions
omikuji config export --format markdown --verbose
```

## Implementation Details

### Format Specifications

#### JSON Export
```json
{
  "version": "1.0",
  "networks": [...],
  "datafeeds": [...],
  "exported_at": "2024-01-15T10:30:00Z",
  "exported_by": "omikuji v0.1.0"
}
```

#### TOML Export
```toml
version = "1.0"
exported_at = "2024-01-15T10:30:00Z"

[[networks]]
name = "ethereum"
rpc_url = "https://eth.llamarpc.com"

[[datafeeds]]
name = "eth_usd"
networks = "ethereum"
```

#### CSV Fields
- Required: name, network, contract_address, contract_type
- Optional: all other fields with sensible defaults

### Import Validation
- Schema validation for all formats
- Type checking and conversion
- Conflict resolution options
- Preview mode before applying changes

## Dependencies
- `csv` for CSV parsing/writing
- `toml` for TOML support
- `serde_json` for JSON handling

## Success Criteria
1. Seamless format conversion
2. Bulk operations via CSV
3. Integration with external tools
4. Data validation on import
5. No data loss during conversion