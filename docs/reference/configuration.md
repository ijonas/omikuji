# Configuration Reference

Complete reference for all Omikuji configuration options.

## Configuration File Format

Omikuji uses YAML format for configuration files. The file must contain two top-level sections: `networks` and `datafeeds`.

## Command Line Options

```bash
omikuji [OPTIONS]
```

### Options

- `-c, --config <FILE>`: Path to configuration file
  - Default: `config.yaml` in current directory, then `~/.omikuji/config.yaml`
  
- `-p, --private-key-env <ENV_VAR>`: Environment variable containing private key
  - Default: `OMIKUJI_PRIVATE_KEY`
  
- `-V, --version`: Display version information

- `-h, --help`: Display help information

## Networks Section

Define blockchain network connections.

```yaml
networks:
  - name: <string>              # Required: Unique network identifier
    rpc_url: <string>           # Required: HTTP(S) RPC endpoint URL
    transaction_type: <string>  # Optional: "legacy" or "eip1559" (default: "eip1559")
    gas_config:                 # Optional: Gas configuration
      <gas_options>
```

### Network Fields

#### `name` (required)
- Type: `string`
- Description: Unique identifier for the network
- Example: `ethereum`, `base`, `polygon`

#### `rpc_url` (required)
- Type: `string`
- Description: HTTP or HTTPS URL for the network's RPC endpoint
- Example: `https://eth.llamarpc.com`

#### `transaction_type` (optional)
- Type: `string`
- Values: `legacy`, `eip1559`
- Default: `eip1559`
- Description: Transaction type to use for this network

#### `gas_config` (optional)
- Type: `object`
- Description: Gas configuration options
- See [Gas Configuration Reference](#gas-configuration) below

## Datafeeds Section

Define data sources and their associated contracts.

```yaml
datafeeds:
  - name: <string>                      # Required: Unique feed identifier
    networks: <string>                  # Required: Network name reference
    check_frequency: <integer>          # Required: Polling interval (seconds)
    contract_address: <string>          # Required: Contract address (0x...)
    contract_type: <string>             # Required: Contract type
    feed_url: <string>                  # Required: Data source URL
    feed_json_path: <string>            # Required: JSON path to value
    
    # Update triggers (at least one required)
    minimum_update_frequency: <integer> # Optional: Time-based trigger (seconds)
    deviation_threshold_pct: <float>    # Optional: Deviation trigger (percent)
    
    # Contract configuration
    read_contract_config: <boolean>     # Optional: Read config from contract
    decimals: <integer>                 # Conditional: Required if read_contract_config=false
    min_value: <number>                 # Optional: Minimum submission value
    max_value: <number>                 # Optional: Maximum submission value
    
    # Additional options
    feed_json_path_timestamp: <string>  # Optional: JSON path to timestamp
```

### Datafeed Fields

#### `name` (required)
- Type: `string`
- Description: Unique identifier for the datafeed
- Example: `eth_usd_price`

#### `networks` (required)
- Type: `string`
- Description: Network name from the networks section
- Example: `ethereum`

#### `check_frequency` (required)
- Type: `integer`
- Range: 1-86400
- Description: How often to poll the data source (seconds)
- Example: `60` (check every minute)

#### `contract_address` (required)
- Type: `string`
- Format: `0x` followed by 40 hexadecimal characters
- Description: Ethereum address of the contract to update
- Example: `0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419`

#### `contract_type` (required)
- Type: `string`
- Values: `fluxmon`
- Description: Type of contract interface
- Note: Currently only Chainlink FluxAggregator is supported

#### `feed_url` (required)
- Type: `string`
- Format: Valid HTTP or HTTPS URL
- Description: API endpoint returning JSON data
- Example: `https://api.coinbase.com/v2/exchange-rates?currency=ETH`

#### `feed_json_path` (required)
- Type: `string`
- Format: Dot-notation path
- Description: Path to extract value from JSON response
- Examples:
  - `price` - Top-level field
  - `data.USD` - Nested field
  - `rates.0.value` - Array access

#### `minimum_update_frequency` (optional)
- Type: `integer`
- Range: 0-2147483647
- Description: Minimum seconds between updates (time-based trigger)
- Example: `3600` (update at least hourly)

#### `deviation_threshold_pct` (optional)
- Type: `float`
- Range: 0.0-100.0
- Description: Percentage change to trigger update
- Example: `0.5` (update on 0.5% change)

#### `read_contract_config` (optional)
- Type: `boolean`
- Default: `true`
- Description: Whether to read decimals/bounds from contract

#### `decimals` (conditional)
- Type: `integer`
- Range: 0-18
- Description: Number of decimal places for the value
- Required if: `read_contract_config = false`

#### `min_value` (optional)
- Type: `number`
- Description: Minimum value the contract will accept
- Default: `0`

#### `max_value` (optional)
- Type: `number`
- Description: Maximum value the contract will accept
- Default: No limit

#### `feed_json_path_timestamp` (optional)
- Type: `string`
- Format: Dot-notation path
- Description: Path to extract Unix timestamp from JSON
- Example: `data.last_updated`

## Gas Configuration

Detailed gas configuration options for each network.

```yaml
gas_config:
  # Fee estimation
  gas_multiplier: <float>           # Multiply estimated gas (default: 1.1)
  max_fee_per_gas: <integer>        # Max fee in gwei (EIP-1559)
  max_priority_fee: <integer>       # Max priority fee in gwei (EIP-1559)
  gas_price: <integer>              # Gas price in gwei (legacy)
  
  # Limits
  gas_limit: <integer>              # Manual gas limit override
  max_gas_price: <integer>          # Maximum gas price in gwei
  
  # Retry behavior
  fee_bump_percentage: <integer>    # Fee increase on retry (default: 10)
  max_retries: <integer>            # Maximum retry attempts (default: 3)
  retry_delay_ms: <integer>         # Delay between retries (default: 5000)
```

See [Gas Configuration Guide](../guides/gas-configuration.md) for detailed explanations.

## Environment Variables

### Required

- `OMIKUJI_PRIVATE_KEY` (or custom via `-p`): Wallet private key

### Optional

- `DATABASE_URL`: PostgreSQL connection string
- `RUST_LOG`: Logging level (`error`, `warn`, `info`, `debug`, `trace`)

## Complete Example

```yaml
# Network definitions
networks:
  - name: ethereum
    rpc_url: https://eth.llamarpc.com
    transaction_type: eip1559
    gas_config:
      gas_multiplier: 1.2
      max_fee_per_gas: 100
      max_priority_fee: 2
      fee_bump_percentage: 15
      max_retries: 5

  - name: base
    rpc_url: https://base.llamarpc.com
    gas_config:
      gas_multiplier: 1.1

# Datafeed definitions
datafeeds:
  # ETH/USD on Ethereum
  - name: eth_usd_mainnet
    networks: ethereum
    check_frequency: 60
    contract_address: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"
    contract_type: fluxmon
    read_contract_config: true
    minimum_update_frequency: 3600
    deviation_threshold_pct: 0.5
    feed_url: https://api.coinbase.com/v2/exchange-rates?currency=ETH
    feed_json_path: data.rates.USD
    
  # BTC/USD on Base with manual config
  - name: btc_usd_base
    networks: base
    check_frequency: 120
    contract_address: "0x64c911996D3c6aC71f9b455B1E8E7266BcbD848F"
    contract_type: fluxmon
    read_contract_config: false
    decimals: 8
    min_value: 0
    max_value: 10000000
    minimum_update_frequency: 7200
    deviation_threshold_pct: 1.0
    feed_url: https://api.coinbase.com/v2/exchange-rates?currency=BTC
    feed_json_path: data.rates.USD
    feed_json_path_timestamp: data.epoch
```

## Validation Rules

1. **Unique Names**: All network and datafeed names must be unique
2. **Network References**: Datafeed networks must reference existing network names
3. **Valid Addresses**: Contract addresses must be valid Ethereum addresses
4. **URL Format**: Feed URLs must be valid HTTP/HTTPS URLs
5. **Update Triggers**: At least one of `minimum_update_frequency` or `deviation_threshold_pct` must be set
6. **Decimal Range**: Decimals must be between 0 and 18
7. **Positive Values**: Frequencies, percentages, and gas values must be positive

## Default Locations

Configuration files are searched in order:
1. Path specified with `-c` flag
2. `./config.yaml` (current directory)
3. `~/.omikuji/config.yaml` (user home directory)

## See Also

- [Configuration Guide](../getting-started/configuration.md) - Basic configuration tutorial
- [Gas Configuration Guide](../guides/gas-configuration.md) - Detailed gas settings
- [Environment Variables Guide](../guides/environment-variables.md) - Security best practices