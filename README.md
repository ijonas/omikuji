# Omikuji - a lightweight EVM blockchain datafeed provider

Omikuji is a software daemon, written in Rust, that provides external off-chain data to EVM blockchains such as Ethereum and BASE.

The core model of Omikuji is the datafeed, which is a Solidity smart contract, that reports a single value and an accompanying timestamp and block number for when that value was last updated.
This allows other (client) smart contracts to ascertain whether or not the value reported by the datafeed has gone stale or not. The concept of 'stale' is arbitrary and completely up to the client smart contracts to define.

## Configuration

The Omikuji daemon process uses a YAML-based configuration file to define its runtime behaviour. The configuration file specifies which datafeeds are maintained and monitored by the daemon process, the external data sources that feed into each datafeed, and the network configuration.

### Sample Configuration

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth.llamarpc.com
  - name: base
    rpc_url: https://base.llamarpc.com

datafeeds:
  - name: eth_usd
    networks: ethereum
    check_frequency: 60
    contract_address: 0x1234567890123456789012345678901234567890
    contract_type: fluxmon
    read_contract_config: true
    minimum_update_frequency: 3600
    deviation_threshold_pct: 0.5
    feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
    feed_json_path: RAW.ETH.USD.PRICE
    feed_json_path_timestamp: RAW.ETH.USD.LASTUPDATE
```

### Configuration Options

#### Networks
- `name`: A unique identifier for the network
- `rpc_url`: The URL for the JSON-RPC endpoint of the blockchain

#### Datafeeds
- `name`: A unique identifier for the datafeed
- `networks`: The network this datafeed operates on (must match a defined network name)
- `check_frequency`: How often to check the datafeed (in seconds)
- `contract_address`: The Ethereum address of the datafeed contract
- `contract_type`: The type of contract (currently supports "fluxmon" for Chainlink Flux Monitor)
- `read_contract_config`: Whether to read configuration from the contract
- `minimum_update_frequency`: Minimum time between updates (in seconds)
- `deviation_threshold_pct`: Threshold percentage deviation to trigger an update
- `feed_url`: URL to fetch the price feed data
- `feed_json_path`: JSON path to extract the price from the feed response
- `feed_json_path_timestamp`: (Optional) JSON path to extract the timestamp from the feed response

## Usage

```bash
# Run with default configuration (~/.omikuji/config.yaml)
cargo run

# Run with specified configuration file
cargo run -- -c config.yaml
```
