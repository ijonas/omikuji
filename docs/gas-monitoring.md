# Gas Monitoring and Analytics

Omikuji provides comprehensive gas consumption monitoring for all blockchain transactions, enabling cost analysis and optimization.

## Overview

Every transaction submitted by Omikuji is tracked with detailed gas metrics:
- Gas limit vs. actual gas used
- Gas price in gwei
- Total transaction cost in native tokens
- Gas efficiency percentage
- Transaction success/failure status

## Features

### Real-time Metrics

Prometheus metrics are exposed at `http://localhost:9090/metrics` including:

- **omikuji_gas_used_total**: Total gas consumed by feed and network
- **omikuji_gas_price_gwei**: Gas price histogram by network and transaction type
- **omikuji_gas_efficiency_percent**: Percentage of gas limit actually used
- **omikuji_transaction_cost_wei**: Transaction cost distribution
- **omikuji_transaction_count**: Number of transactions by status

### Database Storage

All transaction gas data is stored in the `transaction_log` table for historical analysis:

```sql
-- View recent transactions with gas metrics
SELECT 
    feed_name,
    network_name,
    tx_hash,
    gas_used,
    gas_price_gwei,
    total_cost_wei / 1e18 as cost_in_tokens,
    efficiency_percent,
    status,
    created_at
FROM transaction_log
ORDER BY created_at DESC
LIMIT 20;
```

### Efficiency Warnings

Omikuji automatically warns about gas inefficiencies:
- **Low efficiency** (<50%): Gas limit may be too high
- **High usage** (>90%): Gas limit may be too low for safety

## Monitoring Queries

### Daily Gas Costs by Feed
```sql
SELECT * FROM daily_gas_costs
WHERE network_name = 'ethereum'
ORDER BY date DESC;
```

### Transaction Statistics
```sql
SELECT * FROM transaction_stats
ORDER BY total_cost_wei DESC;
```

### High Gas Price Transactions
```sql
SELECT 
    feed_name,
    tx_hash,
    gas_price_gwei,
    total_cost_wei / 1e18 as cost_in_tokens,
    created_at
FROM transaction_log
WHERE gas_price_gwei > 100  -- Transactions over 100 gwei
ORDER BY gas_price_gwei DESC
LIMIT 10;
```

### Inefficient Transactions
```sql
SELECT 
    feed_name,
    tx_hash,
    efficiency_percent,
    gas_limit - gas_used as wasted_gas,
    created_at
FROM transaction_log
WHERE efficiency_percent < 50
    AND status = 'success'
ORDER BY efficiency_percent ASC
LIMIT 10;
```

## Prometheus Integration

### Setting up Grafana Dashboard

1. Add Prometheus data source: `http://localhost:9090`
2. Import the dashboard JSON (see below)
3. Configure alerts for high gas prices or low efficiency

### Example Grafana Dashboard JSON
```json
{
  "dashboard": {
    "title": "Omikuji Gas Metrics",
    "panels": [
      {
        "title": "Gas Price by Network",
        "targets": [{
          "expr": "histogram_quantile(0.95, omikuji_gas_price_gwei)"
        }]
      },
      {
        "title": "Transaction Costs",
        "targets": [{
          "expr": "rate(omikuji_transaction_cost_wei[5m]) / 1e18"
        }]
      },
      {
        "title": "Gas Efficiency",
        "targets": [{
          "expr": "omikuji_gas_efficiency_percent"
        }]
      }
    ]
  }
}
```

## Cost Optimization Tips

### 1. Monitor Gas Price Patterns
```sql
-- Find best times to transact
SELECT 
    EXTRACT(HOUR FROM created_at) as hour,
    AVG(gas_price_gwei) as avg_gas_price,
    COUNT(*) as transaction_count
FROM transaction_log
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY EXTRACT(HOUR FROM created_at)
ORDER BY avg_gas_price ASC;
```

### 2. Optimize Gas Limits
```sql
-- Find optimal gas limits by feed
SELECT 
    feed_name,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY gas_used) as p95_gas_used,
    MAX(gas_used) as max_gas_used,
    AVG(efficiency_percent) as avg_efficiency
FROM transaction_log
WHERE status = 'success'
GROUP BY feed_name;
```

### 3. Network Comparison
```sql
-- Compare costs across networks
SELECT 
    network_name,
    COUNT(*) as transactions,
    AVG(gas_price_gwei) as avg_gas_price,
    SUM(total_cost_wei) / 1e18 as total_cost_tokens
FROM transaction_log
WHERE created_at > NOW() - INTERVAL '30 days'
GROUP BY network_name
ORDER BY total_cost_tokens DESC;
```

## Configuration

### Gas Configuration Options

```yaml
networks:
  - name: ethereum
    gas_config:
      gas_multiplier: 1.2  # 20% safety margin
      fee_bumping:
        enabled: true
        max_retries: 3
        fee_increase_percent: 15.0
```

### Transaction Type Selection

- **Legacy**: Fixed gas price, simpler but less flexible
- **EIP-1559**: Dynamic fees with priority tips, better for congested networks

## Alerting

### High Gas Price Alert
```promql
alert: HighGasPrice
expr: omikuji_gas_price_gwei > 200
for: 5m
labels:
  severity: warning
annotations:
  summary: "High gas price detected on {{ $labels.network }}"
```

### Low Efficiency Alert
```promql
alert: LowGasEfficiency
expr: omikuji_gas_efficiency_percent < 30
for: 10m
labels:
  severity: warning
annotations:
  summary: "Low gas efficiency for {{ $labels.feed_name }}"
```

## Troubleshooting

### No metrics appearing
1. Check metrics endpoint: `curl http://localhost:9090/metrics | grep omikuji`
2. Verify transactions are being submitted
3. Check logs for metric recording errors

### Database queries slow
1. Ensure indexes exist (created by migration)
2. Consider partitioning for large datasets
3. Run `VACUUM ANALYZE transaction_log;`

### High gas costs
1. Review gas price trends
2. Adjust update frequency if possible
3. Consider batching updates
4. Use EIP-1559 for better price discovery