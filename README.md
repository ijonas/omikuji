# Omikuji - a lightweight EVM blockchain datafeed provider

Omikuji is a software daemon, written in Rust, that provides external off-chain data to EVM blockchains such as Ethereum and BASE.

Some may call it a [blockchain oracle](https://en.wikipedia.org/wiki/Blockchain_oracle)

The core model of Omikuji is the datafeed, which is a Solidity smart contract, that reports a single value and an accompanying timestamp and block number for when that value was last updated.

Omikuji will monitor external datafeeds such as price feeds (the price of gold, temperature in London, etc.) and when it notices significant change in the reported data it will write that data to a blockchain.


## ✨ Key Features

  Core Functionality

  - Multi-Network Support: Connect to multiple EVM-compatible blockchains simultaneously
  - Datafeed Management: Define and manage multiple datafeeds through YAML configuration
  - Smart Contract Integration: Full support for Chainlink FluxAggregator contracts
  - Flexible Data Sources: Fetch data from any HTTP/HTTPS JSON API endpoint

  Update Mechanisms

  - Time-Based Updates: Automatically submit new values when minimum update frequency has elapsed
  - Deviation-Based Updates: Submit updates when price changes exceed configured percentage thresholds
  - Dual-Trigger System: Updates occur when either time OR deviation conditions are met

  Configuration

  - Contract Configuration Reading: Automatically read decimals, min/max values from deployed contracts
  - Environment Variable Support: Secure wallet management through environment variables
  - Flexible JSON Path Extraction: Support for complex nested JSON structures using dot notation
  - EIP-1559 Transaction Support: Modern gas pricing with automatic fee estimation
  - Fee Bumping: Automatic retry with increased fees for stuck transactions
  - Gas Configuration: Per-network gas settings with manual override options

  Monitoring & Reliability

  - Concurrent Feed Monitoring: Each datafeed runs independently in its own async task
  - Comprehensive Logging: Detailed logs for debugging and monitoring
  - Error Recovery: Graceful handling of network errors and API failures

## 📋 Requirements

  - Rust 1.70 or higher
  - Access to EVM-compatible blockchain RPC endpoints
  - Private key for transaction signing (via environment variable)


## 🚀 Getting Started

  1. Create a configuration file (default: config.yaml):

    networks:
      - name: local
        url: http://localhost:8545

    datafeeds:
      - name: eth_usd
        networks: local
        check_frequency: 60
        contract_address: 0x...
        contract_type: fluxmon
        minimum_update_frequency: 3600
        deviation_threshold_pct: 0.5
        feed_url: https://api.example.com/price
        feed_json_path: data.price

  2. Set your private key:
    
    export OMIKUJI_PRIVATE_KEY=your_private_key_here

  3. Run Omikuji:
    
    omikuji --config config.yaml

  🔧 Command Line Options

  - -c, --config <FILE>: Path to configuration file
  - -p, --private-key-env <ENV_VAR>: Private key environment variable name (default: OMIKUJI_PRIVATE_KEY)
  - -V, --version: Display version information
  - -h, --help: Display help information

## 📊 Technical Specifications

  - Language: Rust
  - Async Runtime: Tokio
  - Blockchain Library: ethers-rs
  - Configuration Format: YAML
  - Supported Contract Types: Chainlink FluxAggregator
  - Update Precision: Configurable decimals (0-18)
  - Value Bounds: Support for min/max submission values

## 🧪 Testing

  The release includes comprehensive test coverage with 52 unit tests covering:
  - Configuration parsing and validation
  - Contract interaction and ABI encoding
  - JSON data extraction
  - Deviation calculations
  - Network provider management
  - Value scaling and bounds checking

## 🙏 Acknowledgments

  This initial release represents the foundation of the Omikuji project. We look forward to community feedback and contributions to make
  blockchain data feeds more accessible and reliable.

## 📝 License

  Omikuji is licensed under the MIT License. See [LICENSE](LICENSE) for details.
  
  Copyright (c) 2025 Stacking Turtles Ltd.

## 📚 Documentation

  - [Gas Configuration Guide](docs/gas-configuration.md) - Detailed guide for configuring gas settings, transaction types, and fee strategies
  - [Configuration Reference](specs/configuration.md) - Complete configuration file specification

  For more information and contribution guidelines, visit: https://github.com/ijonas/omikuji
