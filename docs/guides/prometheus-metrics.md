# Prometheus Metrics Guide

Omikuji exports comprehensive metrics for monitoring via Prometheus at `http://localhost:9090/metrics`.

## Available Metrics

### Wallet Metrics

- **`omikuji_wallet_balance_wei`** (Gauge)
  - Description: Current wallet balance in wei
  - Labels: `network`, `address`
  - Update frequency: Every 60 seconds
  - Example: `omikuji_wallet_balance_wei{network="mainnet",address="0x123..."} 1234567890000000000`

### Feed Metrics

- **`omikuji_feed_value`** (Gauge)
  - Description: Current value from the external data feed
  - Labels: `feed_name`, `network`
  - Update frequency: Based on feed's `check_frequency` configuration
  - Example: `omikuji_feed_value{feed_name="btc_usd",network="mainnet"} 45123.50`

- **`omikuji_feed_timestamp`** (Gauge)
  - Description: Unix timestamp of the last feed value update
  - Labels: `feed_name`, `network`
  - Example: `omikuji_feed_timestamp{feed_name="btc_usd",network="mainnet"} 1719000000`

### Contract Metrics

- **`omikuji_contract_value`** (Gauge)
  - Description: Current value stored in the on-chain contract
  - Labels: `feed_name`, `network`
  - Update frequency: Checked with each feed poll
  - Example: `omikuji_contract_value{feed_name="btc_usd",network="mainnet"} 45100.00`

- **`omikuji_contract_round`** (Gauge)
  - Description: Current round number from the contract
  - Labels: `feed_name`, `network`
  - Example: `omikuji_contract_round{feed_name="btc_usd",network="mainnet"} 12345`

- **`omikuji_contract_update_timestamp`** (Gauge)
  - Description: Unix timestamp of the last contract update
  - Labels: `feed_name`, `network`
  - Example: `omikuji_contract_update_timestamp{feed_name="btc_usd",network="mainnet"} 1718999900`

- **`omikuji_contract_updates_total`** (Counter)
  - Description: Total number of successful contract updates
  - Labels: `feed_name`, `network`
  - Example: `omikuji_contract_updates_total{feed_name="btc_usd",network="mainnet"} 156`

### Deviation Metrics

- **`omikuji_feed_deviation_percent`** (Gauge)
  - Description: Percentage deviation between feed value and contract value
  - Labels: `feed_name`, `network`
  - Formula: `abs(feed_value - contract_value) / contract_value * 100`
  - Example: `omikuji_feed_deviation_percent{feed_name="btc_usd",network="mainnet"} 0.23`

### Gas Metrics (from Issue #19)

- **`omikuji_gas_used_total`** (Counter)
  - Description: Total gas consumed by transactions
  - Labels: `feed_name`, `network`, `status`
  - Example: `omikuji_gas_used_total{feed_name="btc_usd",network="mainnet",status="success"} 12345678`

- **`omikuji_gas_price_gwei`** (Histogram)
  - Description: Distribution of gas prices paid in gwei
  - Labels: `feed_name`, `network`
  - Buckets: 1, 5, 10, 20, 50, 100, 200, 500 gwei

- **`omikuji_gas_efficiency_percent`** (Histogram)
  - Description: Gas efficiency as percentage of limit used
  - Labels: `feed_name`, `network`
  - Buckets: 10%, 20%, ..., 90%, 95%, 99%, 100%

- **`omikuji_transaction_cost_wei`** (Histogram)
  - Description: Total transaction cost in wei
  - Labels: `feed_name`, `network`

- **`omikuji_transactions_total`** (Counter)
  - Description: Total number of transactions
  - Labels: `feed_name`, `network`, `status`, `tx_type`

## Example PromQL Queries

### Basic Queries

```promql
# Current wallet balance in ETH
omikuji_wallet_balance_wei{network="mainnet"} / 1e18

# Latest feed value for BTC/USD
omikuji_feed_value{feed_name="btc_usd"}

# Deviation percentage for all feeds
omikuji_feed_deviation_percent

# Contract update rate (updates per minute)
rate(omikuji_contract_updates_total[5m]) * 60
```

### Advanced Queries

```promql
# Alert when deviation exceeds 1%
omikuji_feed_deviation_percent > 1

# Average gas price over the last hour
avg_over_time(omikuji_gas_price_gwei[1h])

# Wallet balance changes over time
delta(omikuji_wallet_balance_wei[1h])

# Time since last contract update
time() - omikuji_contract_update_timestamp

# Gas efficiency below 50%
histogram_quantile(0.5, omikuji_gas_efficiency_percent) < 50
```

### Monitoring Alerts

```promql
# Low wallet balance alert (< 0.1 ETH)
omikuji_wallet_balance_wei < 1e17

# Stale feed data (> 10 minutes old)
time() - omikuji_feed_timestamp > 600

# High deviation alert
omikuji_feed_deviation_percent > 2

# Transaction failures
rate(omikuji_transactions_total{status="failed"}[5m]) > 0
```

## Grafana Dashboard Example

Here's a sample Grafana dashboard configuration:

```json
{
  "dashboard": {
    "title": "Omikuji Monitoring",
    "panels": [
      {
        "title": "Wallet Balances",
        "targets": [{
          "expr": "omikuji_wallet_balance_wei / 1e18",
          "legendFormat": "{{network}} - {{address}}"
        }]
      },
      {
        "title": "Feed Deviations",
        "targets": [{
          "expr": "omikuji_feed_deviation_percent",
          "legendFormat": "{{feed_name}} on {{network}}"
        }]
      },
      {
        "title": "Contract Update Rate",
        "targets": [{
          "expr": "rate(omikuji_contract_updates_total[5m]) * 60",
          "legendFormat": "{{feed_name}} updates/min"
        }]
      },
      {
        "title": "Gas Efficiency",
        "targets": [{
          "expr": "histogram_quantile(0.5, omikuji_gas_efficiency_percent)",
          "legendFormat": "Median efficiency {{feed_name}}"
        }]
      }
    ]
  }
}
```

## Best Practices

1. **Monitoring Setup**
   - Set up alerts for low wallet balances to prevent transaction failures
   - Monitor deviation percentages to ensure data accuracy
   - Track gas efficiency to optimize transaction costs

2. **Performance Considerations**
   - Metrics are updated asynchronously and don't impact feed monitoring
   - Wallet balance updates run in a separate task every 60 seconds
   - Contract state is read with each feed poll to minimize RPC calls

3. **Troubleshooting**
   - If metrics are missing, check the logs for errors
   - Ensure the metrics server started successfully on port 9090
   - Verify network connectivity for wallet balance queries

## Integration with Monitoring Stack

1. **Prometheus Configuration**
   ```yaml
   scrape_configs:
     - job_name: 'omikuji'
       static_configs:
         - targets: ['localhost:9090']
       scrape_interval: 15s
   ```

2. **Alertmanager Rules**
   ```yaml
   groups:
     - name: omikuji
       rules:
         - alert: LowWalletBalance
           expr: omikuji_wallet_balance_wei < 1e17
           for: 5m
           annotations:
             summary: "Low wallet balance on {{ $labels.network }}"
         
         - alert: HighDeviation
           expr: omikuji_feed_deviation_percent > 2
           for: 5m
           annotations:
             summary: "High deviation for {{ $labels.feed_name }}"
   ```

## Metric Retention

When metrics are no longer being updated (e.g., a feed is removed), they will remain in Prometheus until:
- The retention period expires (configured in Prometheus)
- The metric is explicitly deleted
- Prometheus is restarted

To handle stale metrics gracefully, use queries that check for recent updates:
```promql
# Only show feeds updated in the last hour
omikuji_feed_value unless (time() - omikuji_feed_timestamp > 3600)
```