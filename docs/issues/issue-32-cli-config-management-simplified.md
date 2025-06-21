# Issue #32: Add Interactive CLI for Configuration Management (Simplified)

## Status
- **Status**: OPEN
- **Type**: Enhancement
- **Priority**: High
- **Labels**: enhancement, feature

## Problem Statement
Currently, users must manually edit YAML configuration files to manage Omikuji feeds and networks. This is:
- Error-prone and difficult to validate
- Hard to manage with multiple feeds
- Lacks version control for configuration changes

## Proposed Solution
Implement a focused interactive CLI system with three core features:

### 1. Interactive Feed Management
Provide CLI commands for basic CRUD operations on feeds:

```bash
# Add a new feed interactively
omikuji feed add --interactive

# Update an existing feed
omikuji feed update eth_usd --deviation-threshold 1.0
omikuji feed update eth_usd --check-frequency 120

# Delete a feed
omikuji feed delete eth_usd

# List all feeds
omikuji feed list
omikuji feed list --network ethereum

# Show feed details
omikuji feed show eth_usd
```

**Interactive Mode Features:**
- Step-by-step prompts for all required fields
- Validation of inputs (addresses, URLs, numeric values)
- Preview changes before applying
- Automatic config file backup before modifications

### 2. Configuration Versioning
Simple versioning system for configuration changes:

```bash
# View configuration history
omikuji config history
omikuji config history --limit 10

# Show specific version
omikuji config show --version 2024-01-15_14-30-00

# Rollback to previous version
omikuji config rollback
omikuji config rollback --version 2024-01-15_14-30-00

# Compare versions
omikuji config diff --from 1 --to 2
```

**Implementation Details:**
- Automatic backup on every change
- Store versions in `.omikuji/config-history/`
- Maximum 50 versions by default (configurable)
- Include timestamp and change summary

### 3. Configuration Linting
Built-in linting to catch common issues:

```bash
# Lint current configuration
omikuji config lint

# Lint specific file
omikuji config lint --file custom-config.yaml

# Auto-fix common issues
omikuji config lint --fix
```

**Linting Rules:**
- Check for duplicate feed names
- Validate contract addresses (correct format and checksum)
- Verify RPC URLs are reachable
- Check deviation thresholds are reasonable (0.1% - 10%)
- Ensure update frequencies make sense (>30s, <24h)
- Validate JSON paths exist in feed URLs
- Check for unused network configurations
- Warn about missing optional fields that improve reliability

## Implementation Details

### Dependencies
```toml
[dependencies]
clap = { version = "4.0", features = ["derive"] }
inquire = "0.6"  # For interactive prompts
comfy-table = "7.0"  # For formatted output
serde_json = "1.0"  # For config serialization
```

### Command Structure
```
omikuji
├── feed
│   ├── add [--interactive]
│   ├── update <name> [options]
│   ├── delete <name>
│   ├── list [--network <network>]
│   └── show <name>
├── config
│   ├── lint [--file <path>] [--fix]
│   ├── history [--limit <n>]
│   ├── show [--version <version>]
│   ├── rollback [--version <version>]
│   └── diff [--from <v1>] [--to <v2>]
└── [existing commands]
```

### File Structure
```
.omikuji/
├── config-history/
│   ├── 2024-01-15_14-30-00.yaml
│   ├── 2024-01-15_15-45-30.yaml
│   └── ...
└── config-history.json  # Metadata about changes
```

## Success Criteria
1. Users can manage feeds without manually editing YAML
2. Configuration changes are tracked and reversible
3. Common configuration errors are caught before runtime
4. Backward compatibility with existing YAML files
5. Clear, helpful error messages and prompts

## Related Issues
This issue has been simplified from the original comprehensive proposal. The following features have been moved to separate sub-issues:
- #33: Validation & Testing Features
- #34: Template System
- #35: Import/Export Functionality
- #36: Environment Management
- #37: Advanced Features (multi-file, hot reload, encryption)
- #38: Shell Integration
- #39: Backup & Sync
- #40: Monitoring Features

## Timeline
- Week 1-2: Implement interactive feed management
- Week 3: Add configuration versioning
- Week 4: Implement configuration linting
- Week 5: Testing and documentation