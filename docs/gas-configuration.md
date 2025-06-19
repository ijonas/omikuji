# Gas Configuration Guide

This guide explains how to configure gas settings for Omikuji when interacting with different EVM blockchain networks. Omikuji supports both legacy and EIP-1559 transaction formats with automatic gas estimation and fee bumping capabilities.

## Table of Contents
- [Overview](#overview)
- [Basic Configuration](#basic-configuration)
- [Transaction Types](#transaction-types)
- [Gas Estimation](#gas-estimation)
- [Fee Bumping](#fee-bumping)
- [Configuration Examples](#configuration-examples)
- [Common Scenarios](#common-scenarios)
- [Troubleshooting](#troubleshooting)

## Overview

Omikuji's gas configuration system provides:
- Support for both legacy and EIP-1559 transaction formats
- Automatic gas price estimation with manual override options
- Configurable safety margins through gas multipliers
- Automatic retry with fee bumping for stuck transactions
- Per-network configuration flexibility

## Basic Configuration

Gas settings are configured per network in your `config.yaml` file:

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559  # or "legacy"
    gas_config:
      # All gas configuration options (see below)
```

## Transaction Types

### EIP-1559 Transactions (Recommended)

EIP-1559 is the modern transaction format introduced in Ethereum's London upgrade. It provides more predictable gas pricing and better user experience.

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559  # Default value
    gas_config:
      max_fee_per_gas_gwei: 50.0        # Maximum fee you're willing to pay
      max_priority_fee_per_gas_gwei: 2.0 # Tip for miners/validators
```

### Legacy Transactions

Some networks or older systems may require legacy transaction format:

```yaml
networks:
  - name: bsc
    rpc_url: https://bsc-dataseed.binance.org/
    transaction_type: legacy
    gas_config:
      gas_price_gwei: 5.0  # Fixed gas price for legacy transactions
```

## Gas Estimation

### Automatic Estimation (Default)

By default, Omikuji automatically estimates gas prices and limits:

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559
    gas_config:
      gas_multiplier: 1.2  # 20% safety margin (default)
```

### Manual Gas Limit

Override automatic gas limit estimation:

```yaml
gas_config:
  gas_limit: 300000  # Fixed gas limit
```

### Gas Multiplier

The gas multiplier adds a safety margin to estimated values:

```yaml
gas_config:
  gas_multiplier: 1.5  # 50% safety margin
```

This multiplier applies to:
- Estimated gas limits
- Estimated gas prices (legacy)
- Estimated max fees (EIP-1559)

## Fee Bumping

Omikuji can automatically retry stuck transactions with increased fees:

```yaml
gas_config:
  fee_bumping:
    enabled: true               # Enable automatic retries (default)
    max_retries: 3             # Maximum retry attempts
    initial_wait_seconds: 30   # Wait time before first retry
    fee_increase_percent: 10.0 # Fee increase per retry
```

### How Fee Bumping Works

1. Transaction is sent with initial gas settings
2. If not confirmed within `initial_wait_seconds`, increase fees by `fee_increase_percent`
3. Retry up to `max_retries` times
4. Each retry increases fees cumulatively

Example progression with 10% increase:
- Attempt 1: 20 gwei
- Attempt 2: 22 gwei (10% increase)
- Attempt 3: 24 gwei (20% increase)
- Attempt 4: 26 gwei (30% increase)

## Configuration Examples

### Example 1: Ethereum Mainnet (High Traffic)

For Ethereum mainnet during high traffic periods:

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559
    gas_config:
      # Higher multiplier for congested network
      gas_multiplier: 1.5
      
      # Manual overrides for critical feeds
      max_fee_per_gas_gwei: 100.0
      max_priority_fee_per_gas_gwei: 3.0
      
      # Aggressive retry strategy
      fee_bumping:
        enabled: true
        max_retries: 5
        initial_wait_seconds: 20
        fee_increase_percent: 15.0
```

### Example 2: Layer 2 Network (Low Fees)

For Layer 2 networks like Arbitrum or Optimism:

```yaml
networks:
  - name: arbitrum
    rpc_url: https://arb1.arbitrum.io/rpc
    transaction_type: eip1559
    gas_config:
      # Lower multiplier for stable L2
      gas_multiplier: 1.1
      
      # Let automatic estimation handle it
      # No manual overrides needed
      
      # Less aggressive retry
      fee_bumping:
        enabled: true
        max_retries: 2
        initial_wait_seconds: 10
        fee_increase_percent: 20.0
```

### Example 3: Private/Test Network

For private networks or testnets:

```yaml
networks:
  - name: localhost
    rpc_url: http://localhost:8545
    transaction_type: legacy  # Many test networks use legacy
    gas_config:
      gas_price_gwei: 1.0     # Fixed low price
      gas_limit: 200000       # Fixed limit
      
      # Disable retries for testing
      fee_bumping:
        enabled: false
```

### Example 4: BSC (Binance Smart Chain)

For BSC with its stable, low fees:

```yaml
networks:
  - name: bsc
    rpc_url: https://bsc-dataseed.binance.org/
    transaction_type: legacy  # BSC often uses legacy format
    gas_config:
      gas_price_gwei: 5.0     # BSC typical gas price
      gas_multiplier: 1.1     # Small safety margin
      
      fee_bumping:
        enabled: true
        max_retries: 2
        initial_wait_seconds: 5  # BSC has fast blocks
        fee_increase_percent: 10.0
```

## Common Scenarios

### Scenario 1: "My transactions are failing during network congestion"

**Solution**: Increase gas multiplier and enable aggressive fee bumping

```yaml
gas_config:
  gas_multiplier: 2.0  # Double the estimated gas
  fee_bumping:
    enabled: true
    max_retries: 5
    initial_wait_seconds: 30
    fee_increase_percent: 20.0  # 20% increase per retry
```

### Scenario 2: "I want to minimize gas costs on a stable network"

**Solution**: Use lower multipliers and manual limits

```yaml
gas_config:
  gas_multiplier: 1.05  # Only 5% safety margin
  gas_limit: 150000     # Set precise limit if known
  fee_bumping:
    enabled: true
    max_retries: 1      # Minimal retries
    fee_increase_percent: 5.0
```

### Scenario 3: "I need guaranteed execution for critical price feeds"

**Solution**: Set high manual fees and aggressive retries

```yaml
gas_config:
  # EIP-1559 with high limits
  max_fee_per_gas_gwei: 200.0      # High ceiling
  max_priority_fee_per_gas_gwei: 5.0  # Good tip
  gas_multiplier: 1.5
  
  fee_bumping:
    enabled: true
    max_retries: 10     # Many retries
    initial_wait_seconds: 15
    fee_increase_percent: 25.0  # Aggressive increases
```

### Scenario 4: "Different datafeeds have different priorities"

While gas is configured per-network, you can run multiple Omikuji instances with different configurations:

**High-priority feeds** (e.g., ETH/USD):
```yaml
# config-high-priority.yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559
    gas_config:
      max_fee_per_gas_gwei: 150.0
      max_priority_fee_per_gas_gwei: 4.0
```

**Low-priority feeds** (e.g., less critical pairs):
```yaml
# config-low-priority.yaml
networks:
  - name: ethereum
    rpc_url: https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
    transaction_type: eip1559
    gas_config:
      gas_multiplier: 1.1  # Just use estimated prices
```

## Troubleshooting

### Transaction Failures

If transactions are failing, check:

1. **Insufficient gas limit**: Increase `gas_multiplier` or set manual `gas_limit`
2. **Low gas price**: For legacy, increase `gas_price_gwei`. For EIP-1559, increase `max_fee_per_gas_gwei`
3. **Network rejection**: Some networks have minimum gas prices. Check network requirements.

### Stuck Transactions

If transactions get stuck:

1. Ensure `fee_bumping.enabled: true`
2. Decrease `initial_wait_seconds` for faster retries
3. Increase `fee_increase_percent` for larger bumps
4. Check if your `max_fee_per_gas_gwei` ceiling is too low

### High Gas Costs

To reduce gas costs:

1. Lower `gas_multiplier` (but risk more failures)
2. Use automatic estimation instead of manual overrides
3. For EIP-1559, lower `max_priority_fee_per_gas_gwei`
4. Consider batching updates or increasing `minimum_update_frequency`

### Logs and Monitoring

Omikuji logs all gas-related activities:

```
[INFO] EIP-1559 fees: max_fee=45.2 gwei, priority_fee=2.0 gwei (1.2x multiplier applied)
[INFO] Sending transaction (attempt 1)
[INFO] Transaction sent: 0x123...
[WARN] Transaction timed out after 30 seconds: 0x123...
[INFO] Bumped EIP-1559 fees to: max_fee=49.72 gwei, priority_fee=2.2 gwei
[INFO] Sending transaction (attempt 2)
[INFO] Transaction confirmed: 0x456..., gas used: 125000
```

## Best Practices

1. **Start with defaults**: The default configuration works well for most networks
2. **Monitor and adjust**: Watch logs and adjust multipliers based on success rates
3. **Use EIP-1559 when possible**: It provides better UX and more predictable pricing
4. **Set reasonable retry limits**: Balance between reliability and cost
5. **Test configuration changes**: Use testnets to verify settings before mainnet
6. **Keep some headroom**: Better to overpay slightly than have failed transactions

## Default Values Reference

If not specified, these defaults apply:

```yaml
transaction_type: "eip1559"
gas_config:
  gas_limit: null  # Auto-estimate
  gas_price_gwei: null  # Auto-estimate
  max_fee_per_gas_gwei: null  # Auto-estimate
  max_priority_fee_per_gas_gwei: null  # Auto-estimate
  gas_multiplier: 1.2
  fee_bumping:
    enabled: true
    max_retries: 3
    initial_wait_seconds: 30
    fee_increase_percent: 10.0
```