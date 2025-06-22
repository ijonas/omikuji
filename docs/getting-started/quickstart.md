# Quick Start Tutorial

Get Omikuji running with a live datafeed in 5 minutes. This tutorial will set up a local development environment and create your first price feed.

## Prerequisites

- Omikuji installed ([Installation Guide](installation.md))
- Node.js and npm (for running Anvil)
- A text editor

## Step 1: Start a Local Blockchain

We'll use Anvil (from Foundry) for local development:

```bash
# Install Anvil if you haven't already
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Start Anvil with a known mnemonic
anvil --mnemonic "test test test test test test test test test test test junk"
```

This starts a local blockchain at `http://localhost:8545` with funded test accounts.

## Step 2: Deploy a Test Contract

For this quickstart, we'll use a pre-deployed Chainlink FluxAggregator contract. In production, you would deploy your own or use existing contracts.

Save this as `deploy-test-contract.js`:

```javascript
// Simple example - in production use proper deployment tools
console.log("Contract deployment example:");
console.log("Address: 0x5FbDB2315678afecb367f032d93F642f64180aa3");
console.log("(Deploy your FluxAggregator contract here)");
```

## Step 3: Create Configuration

Create a file named `quickstart-config.yaml`:

```yaml
# Local test network
networks:
  - name: local
    rpc_url: http://localhost:8545
    transaction_type: legacy

# ETH/USD price feed
datafeeds:
  - name: eth_usd_test
    networks: local
    check_frequency: 30                    # Check every 30 seconds
    contract_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3"
    contract_type: fluxmon
    read_contract_config: false            # Manual config for quickstart
    decimals: 8
    min_value: 0
    max_value: 1000000
    minimum_update_frequency: 60           # Update at least every minute
    deviation_threshold_pct: 0.1           # Update on 0.1% change
    feed_url: https://api.coinbase.com/v2/exchange-rates?currency=ETH
    feed_json_path: data.rates.USD
```

## Step 4: Set Up Your Wallet

Use one of Anvil's test private keys:

```bash
# Test private key from Anvil (Account #0)
export OMIKUJI_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

**⚠️ WARNING**: Never use test keys on mainnet!

## Step 5: Run Omikuji

Start Omikuji with your configuration:

```bash
omikuji -c quickstart-config.yaml
```

You should see output like:

```
[2024-01-15T10:30:00Z INFO  omikuji] Using configuration file: "quickstart-config.yaml"
[2024-01-15T10:30:00Z INFO  omikuji] Loaded 1 network(s) and 1 datafeed(s)
[2024-01-15T10:30:00Z INFO  omikuji] Network: local (http://localhost:8545)
[2024-01-15T10:30:00Z INFO  omikuji] Datafeed: eth_usd_test on network local
[2024-01-15T10:30:01Z INFO  omikuji] Starting feed monitor: eth_usd_test
[2024-01-15T10:30:01Z INFO  omikuji] Fetched value 3245.67 at timestamp 1705318201
```

## Step 6: Monitor Your Feed

Omikuji will now:
1. Check the price every 30 seconds
2. Update the contract when:
   - Price changes by 0.1% or more
   - OR 60 seconds have passed since last update

### View Metrics

Omikuji exposes Prometheus metrics at `http://localhost:9090/metrics`:

```bash
# Check feed value
curl -s http://localhost:9090/metrics | grep feed_value

# Check update count
curl -s http://localhost:9090/metrics | grep feed_updates
```

## Step 7: Test an Update

To force an update, you can:

1. **Wait**: After 60 seconds, time-based update will trigger
2. **Price Change**: If ETH price changes by 0.1%, deviation-based update triggers

Watch the logs to see updates:

```
[2024-01-15T10:31:01Z INFO  omikuji] Submitting update: value=3248.92, roundId=1
[2024-01-15T10:31:02Z INFO  omikuji] Transaction sent: 0x123...
[2024-01-15T10:31:02Z INFO  omikuji] Update confirmed in block 42
```

## Next Steps

Now that you have Omikuji running:

### 1. Use Real Networks

Update your config to use testnet:

```yaml
networks:
  - name: sepolia
    rpc_url: https://rpc.sepolia.org
    gas_config:
      gas_multiplier: 1.2
```

### 2. Add Multiple Feeds

```yaml
datafeeds:
  - name: eth_usd
    # ... config ...
    
  - name: btc_usd
    # ... config ...
```

### 3. Enable Database

Store historical data:

```bash
export DATABASE_URL=postgresql://localhost/omikuji
```

See [Database Setup](../guides/database-setup.md) for details.

### 4. Production Deployment

- Use environment variables for sensitive data
- Set up monitoring with Prometheus
- Configure appropriate gas settings
- Use systemd or Docker for process management

## Troubleshooting

### "Contract not found"
- Ensure contract is deployed at the specified address
- Check you're connected to the right network

### "Insufficient funds"
- Ensure your wallet has ETH for gas
- Check the private key is correct

### "Cannot connect to RPC"
- Verify the RPC URL is correct
- Check if Anvil is still running
- Try `curl http://localhost:8545` to test

### "Invalid JSON path"
- Test your API endpoint: `curl https://api.coinbase.com/v2/exchange-rates?currency=ETH`
- Verify the JSON structure matches your path

## Summary

You've successfully:
- ✅ Configured a local network
- ✅ Set up a price feed
- ✅ Run Omikuji with live data
- ✅ Monitored feed updates

For production use, see:
- [Complete Configuration Reference](../reference/configuration.md)
- [Gas Configuration Guide](../guides/gas-configuration.md)
- [Production Deployment](../guides/production-deployment.md)