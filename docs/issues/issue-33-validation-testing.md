# Issue #33: CLI Validation & Testing Features

## Status
- **Status**: OPEN
- **Type**: Enhancement
- **Parent Issue**: #32
- **Labels**: enhancement, feature, cli

## Overview
Add comprehensive validation and testing capabilities to the Omikuji CLI to ensure configurations work correctly before deployment.

## Proposed Features

### 1. Configuration Validation
Deep validation of configuration files:

```bash
# Validate entire configuration
omikuji config validate

# Validate with verbose output
omikuji config validate --verbose

# Validate specific sections
omikuji config validate --only networks
omikuji config validate --only feeds
```

**Validation Checks:**
- Contract exists on specified network
- Contract is accessible and has correct ABI
- RPC endpoints are reachable
- Feed URLs return valid JSON
- JSON paths exist in feed responses
- Private key has sufficient balance
- Gas settings are reasonable

### 2. Feed Connectivity Testing
Test feeds without submitting transactions:

```bash
# Test all feeds
omikuji feed test --all

# Test specific feed
omikuji feed test eth_usd

# Test with detailed output
omikuji feed test eth_usd --verbose
```

**Test Operations:**
- Fetch data from feed URL
- Parse JSON response
- Extract value using JSON path
- Validate value is numeric and within bounds
- Check contract connection
- Simulate value submission (dry run)

### 3. Dry Run Mode
Execute full update cycle without submitting transactions:

```bash
# Dry run for all feeds
omikuji run --dry-run

# Dry run for specific duration
omikuji run --dry-run --duration 300

# Dry run with transaction simulation
omikuji run --dry-run --simulate-tx
```

**Dry Run Features:**
- Full update cycle simulation
- Gas estimation without submission
- Log all actions that would be taken
- Report potential issues
- Generate cost estimates

## Implementation Details

### Command Structure
```
omikuji
├── config
│   └── validate [--verbose] [--only <section>]
├── feed
│   └── test <name> [--all] [--verbose]
└── run
    └── --dry-run [--duration <seconds>] [--simulate-tx]
```

### Output Format
```
Validation Results:
✓ Network 'ethereum' RPC is reachable
✓ Contract 0x... exists and is accessible
✗ Feed URL returns 404
✓ JSON path 'data.price' exists
⚠ Warning: Gas price seems high (150 gwei)

Summary: 3 passed, 1 failed, 1 warning
```

## Dependencies
- `reqwest` for HTTP testing
- `ethers` for contract validation
- `colored` for terminal output

## Success Criteria
1. Users can validate configurations before running
2. Clear identification of configuration issues
3. Ability to test individual components
4. No accidental transactions during testing