# Configuration Guide

This guide covers the basic configuration needed to get Omikuji running with your first datafeed.

## Configuration File

Omikuji uses a YAML configuration file to define networks and datafeeds. By default, it looks for `config.yaml` in the current directory, but you can specify a different path with the `-c` flag.

## Basic Structure

A minimal configuration file has two main sections:

```yaml
networks:
  # Define blockchain networks
  
datafeeds:
  # Define data sources and contracts
```

## Network Configuration

Networks define the blockchain connections:

```yaml
networks:
  - name: ethereum        # Unique identifier
    rpc_url: https://eth.llamarpc.com    # RPC endpoint
    transaction_type: eip1559             # Optional: "legacy" or "eip1559" (default)
    gas_config:                           # Optional: gas settings
      gas_multiplier: 1.2                 # Multiply estimated gas by this factor
```

### Common Networks

Here are example configurations for popular networks:

```yaml
networks:
  # Ethereum Mainnet
  - name: ethereum
    rpc_url: https://eth.llamarpc.com
    
  # Base
  - name: base
    rpc_url: https://base.llamarpc.com
    
  # Local Development (Anvil/Hardhat)
  - name: local
    rpc_url: http://localhost:8545
    transaction_type: legacy    # Often needed for local networks
```

## Datafeed Configuration

Datafeeds define what data to fetch and where to send it:

```yaml
datafeeds:
  - name: eth_usd                    # Unique identifier
    networks: ethereum               # Which network to use
    check_frequency: 60              # How often to check for updates (seconds)
    contract_address: "0x..."        # Smart contract address
    contract_type: fluxmon           # Contract type (currently only fluxmon)
    minimum_update_frequency: 3600   # Minimum time between updates (seconds)
    deviation_threshold_pct: 0.5     # Update if price deviates by this %
    feed_url: https://api.example.com/price
    feed_json_path: data.price       # Path to value in JSON response
```

### Required Fields

- `name`: Unique identifier for the datafeed
- `networks`: Network name from the networks section
- `check_frequency`: How often to poll the data source (seconds)
- `contract_address`: Ethereum address of the contract to update
- `contract_type`: Type of contract (currently only "fluxmon" supported)
- `feed_url`: HTTP(S) endpoint that returns JSON data
- `feed_json_path`: Dot-notation path to extract value from JSON

### Update Triggers

Updates are triggered by either time OR deviation:

- **Time-based**: Updates when `minimum_update_frequency` seconds have passed
- **Deviation-based**: Updates when value changes by `deviation_threshold_pct` percent

### Contract Configuration

You can either read configuration from the contract or specify it manually:

```yaml
# Option 1: Read from contract (recommended)
read_contract_config: true

# Option 2: Manual configuration
read_contract_config: false
decimals: 8                  # Number of decimal places
min_value: 0                # Minimum allowed value
max_value: 1000000          # Maximum allowed value
```

## JSON Path Examples

The `feed_json_path` uses dot notation to extract values:

```yaml
# Simple path
feed_json_path: price              # {"price": 100.50}

# Nested path
feed_json_path: data.USD.price     # {"data": {"USD": {"price": 100.50}}}

# Array access (0-indexed)
feed_json_path: prices.0.value     # {"prices": [{"value": 100.50}]}
```

### Optional Timestamp

You can extract timestamps from the API response:

```yaml
feed_json_path_timestamp: data.timestamp    # Unix timestamp from API
```

If not specified, Omikuji uses the current time.

## Environment Variables

Sensitive data should use environment variables:

### Private Key

```bash
# Default variable name
export OMIKUJI_PRIVATE_KEY=your_private_key_here

# Or use custom variable
export MY_WALLET_KEY=your_private_key_here
omikuji -c config.yaml -p MY_WALLET_KEY
```

### Database URL (Optional)

```bash
export DATABASE_URL=postgresql://user:pass@localhost/omikuji
```

## Complete Example

Here's a complete configuration for monitoring ETH/USD price:

```yaml
# Networks configuration
networks:
  - name: ethereum
    rpc_url: https://eth.llamarpc.com
    transaction_type: eip1559
    gas_config:
      gas_multiplier: 1.1

# Datafeeds configuration
datafeeds:
  - name: eth_usd_price
    networks: ethereum
    check_frequency: 60                    # Check every minute
    contract_address: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"
    contract_type: fluxmon
    read_contract_config: true             # Read decimals from contract
    minimum_update_frequency: 3600         # Update at least hourly
    deviation_threshold_pct: 0.5           # Update on 0.5% change
    feed_url: https://min-api.cryptocompare.com/data/price?fsym=ETH&tsyms=USD
    feed_json_path: USD
```

## Configuration File Location

Omikuji looks for configuration in this order:

1. Path specified with `-c` flag
2. `config.yaml` in current directory
3. `~/.omikuji/config.yaml` (user home directory)

## Validation

Omikuji validates configuration on startup and will report:
- Missing required fields
- Invalid network references
- Malformed addresses
- Invalid URLs

## Next Steps

- [Quick Start Tutorial](quickstart.md) - Run your first datafeed
- [Gas Configuration](../guides/gas-configuration.md) - Advanced gas settings
- [Complete Reference](../reference/configuration.md) - All configuration options

## Tips

1. **Start Simple**: Begin with one network and one datafeed
2. **Test Locally**: Use Anvil or Hardhat for development
3. **Monitor Logs**: Run with `RUST_LOG=debug` for detailed output
4. **Check Contract**: Ensure your wallet has permission to update the contract