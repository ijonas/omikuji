# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Omikuji is a lightweight EVM blockchain datafeed provider, written in Rust. It acts as a software daemon that provides external off-chain data to EVM blockchains such as Ethereum and BASE.

The core concept is the "datafeed" - a Solidity smart contract that reports a single value along with a timestamp and block number indicating when that value was last updated. This allows other client smart contracts to determine whether the datafeed values have become stale.

## Architecture

### Key Components

1. **Datafeed Management**: Omikuji manages datafeeds defined in YAML configuration files, each with sources, update frequency, and deviation thresholds.

2. **Network Support**: Supports multiple EVM blockchain networks (Ethereum, BASE, etc.) configured with RPC endpoints.

3. **Smart Contract Integration**: Specifically supports Chainlink Flux Monitor contracts for updating datafeed values, utilizing the FluxAggregator interface.

4. **Web Interface**: Provides a dashboard to monitor datafeed status at http://localhost:8080.

### Configuration

The system uses a YAML configuration file that defines:
- Networks with their RPC URLs
- Datafeeds with parameters such as:
  - Check frequency
  - Contract addresses and types
  - Minimum update frequency
  - Deviation thresholds for updates
  - External data source URLs and JSON paths

## Development Commands

### Building and Running

```bash
# Build the project
cargo build

# Run in development mode
cargo run

# Run with specific configuration file
cargo run -- -c /path/to/config.yaml

# Run with release optimizations
cargo build --release
cargo run --release
```

### Testing

```bash
# Run all tests
cargo test

# Run specific tests
cargo test <test_name>

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Check code formatting
cargo fmt --check

# Format code
cargo fmt

# Run clippy linter
cargo clippy

# Check for common mistakes and improvements
cargo clippy -- -D warnings
```

### Documentation

```bash
# Generate documentation
cargo doc --open
```

## Project Documentation

For comprehensive project documentation, see:
- [Documentation Index](docs/README.md) - Complete documentation overview
- [Architecture Reference](docs/reference/architecture.md) - System design details
- [Configuration Reference](docs/reference/configuration.md) - All configuration options
- [Contributing Guide](docs/development/contributing.md) - Development guidelines