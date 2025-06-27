# Key Storage Monitoring Guide

This guide covers monitoring and alerting for Omikuji's key storage backends to ensure security and reliability.

## Key Metrics to Monitor

### Performance Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Key retrieval latency | Time to fetch a key | > 1 second |
| Cache hit rate | Percentage of requests served from cache | < 80% |
| Backend availability | Successful connections to storage backend | < 99% |
| Operation errors | Failed key operations | > 5 per minute |

### Security Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Failed authentication | Invalid credentials or tokens | > 3 per minute |
| Unauthorized access | Access denied errors | Any occurrence |
| Key access frequency | Unusual access patterns | > 2x normal |
| New key creation | Unexpected key additions | Any unplanned |

## Prometheus Metrics

Omikuji exposes the following Prometheus metrics for key storage:

```prometheus
# Key operation counter
omikuji_key_operations_total{operation="get|store|remove|list", backend="keyring|vault|aws", status="success|failure"}

# Operation duration histogram
omikuji_key_operation_duration_seconds{operation="get|store|remove|list", backend="keyring|vault|aws"}

# Cache metrics
omikuji_key_cache_hits_total{backend="vault|aws"}
omikuji_key_cache_misses_total{backend="vault|aws"}
omikuji_key_cache_size{backend="vault|aws"}

# Backend health
omikuji_key_backend_up{backend="keyring|vault|aws"}
```

### Example Prometheus Queries

**Key retrieval error rate:**
```promql
rate(omikuji_key_operations_total{operation="get", status="failure"}[5m]) 
/ 
rate(omikuji_key_operations_total{operation="get"}[5m])
```

**Average operation latency:**
```promql
histogram_quantile(0.95, 
  rate(omikuji_key_operation_duration_seconds_bucket[5m])
)
```

**Cache effectiveness:**
```promql
omikuji_key_cache_hits_total / 
(omikuji_key_cache_hits_total + omikuji_key_cache_misses_total)
```

## Grafana Dashboard

Create a Grafana dashboard with these panels:

### Key Operations Panel
```json
{
  "title": "Key Operations Rate",
  "targets": [{
    "expr": "rate(omikuji_key_operations_total[5m])",
    "legendFormat": "{{operation}} - {{backend}} - {{status}}"
  }]
}
```

### Latency Panel
```json
{
  "title": "Operation Latency (p95)",
  "targets": [{
    "expr": "histogram_quantile(0.95, rate(omikuji_key_operation_duration_seconds_bucket[5m]))",
    "legendFormat": "{{operation}} - {{backend}}"
  }]
}
```

### Cache Performance Panel
```json
{
  "title": "Cache Hit Rate",
  "targets": [{
    "expr": "rate(omikuji_key_cache_hits_total[5m]) / (rate(omikuji_key_cache_hits_total[5m]) + rate(omikuji_key_cache_misses_total[5m]))",
    "legendFormat": "{{backend}}"
  }]
}
```

## Backend-Specific Monitoring

### HashiCorp Vault

**Vault Audit Logs:**
```bash
# Enable audit logging
vault audit enable file file_path=/var/log/vault/audit.log

# Parse audit logs for monitoring
tail -f /var/log/vault/audit.log | jq '. | select(.type == "response" and .error != null)'
```

**Vault Metrics:**
- Monitor via Vault's `/v1/sys/metrics` endpoint
- Key metrics: `vault.token.lookup`, `vault.kv.read`, `vault.kv.write`

### AWS Secrets Manager

**CloudWatch Metrics:**
```yaml
# Terraform example for CloudWatch alarm
resource "aws_cloudwatch_metric_alarm" "high_secret_access" {
  alarm_name          = "omikuji-high-secret-access"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "NumberOfAPICallsForSecrets"
  namespace           = "AWS/SecretsManager"
  period              = "300"
  statistic           = "Sum"
  threshold           = "100"
  
  dimensions = {
    SecretName = "omikuji/*"
  }
}
```

**CloudTrail Analysis:**
```bash
# Query CloudTrail logs for secret access
aws cloudtrail lookup-events \
  --lookup-attributes AttributeKey=ResourceName,AttributeValue=omikuji \
  --start-time $(date -u -d '1 hour ago' +%s) \
  --query 'Events[?EventName==`GetSecretValue`]'
```

## Alert Rules

### Prometheus Alerting Rules

```yaml
groups:
  - name: omikuji_key_storage
    rules:
      - alert: HighKeyRetrievalLatency
        expr: histogram_quantile(0.95, rate(omikuji_key_operation_duration_seconds_bucket{operation="get"}[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High key retrieval latency"
          description: "95th percentile latency is {{ $value }}s"
      
      - alert: KeyStorageBackendDown
        expr: omikuji_key_backend_up == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Key storage backend is down"
          description: "{{ $labels.backend }} backend is not responding"
      
      - alert: LowCacheHitRate
        expr: |
          rate(omikuji_key_cache_hits_total[5m]) / 
          (rate(omikuji_key_cache_hits_total[5m]) + rate(omikuji_key_cache_misses_total[5m])) < 0.8
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Low cache hit rate"
          description: "Cache hit rate is {{ $value | humanizePercentage }}"
      
      - alert: UnauthorizedKeyAccess
        expr: increase(omikuji_key_operations_total{status="failure"}[5m]) > 5
        labels:
          severity: critical
        annotations:
          summary: "Multiple failed key access attempts"
          description: "{{ $value }} failed attempts in the last 5 minutes"
```

## Logging Best Practices

### Structured Logging

Configure Omikuji to output structured logs:

```yaml
# In your omikuji.yaml
logging:
  format: json
  level: info
  targets:
    - type: file
      path: /var/log/omikuji/app.log
    - type: stdout
```

### Log Aggregation

Use tools like Elasticsearch, Loki, or CloudWatch Logs:

**Loki Example:**
```yaml
# promtail config for Omikuji logs
clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: omikuji
    static_configs:
      - targets:
          - localhost
        labels:
          job: omikuji
          __path__: /var/log/omikuji/*.log
    pipeline_stages:
      - json:
          expressions:
            level: level
            operation: operation
            network: network
            backend: storage_type
      - labels:
          level:
          operation:
          backend:
```

### Important Log Queries

**Failed key operations:**
```logql
{job="omikuji"} |= "Key storage operation" | json | status="false"
```

**Audit trail for specific network:**
```logql
{job="omikuji"} |= "audit" | json | network="ethereum-mainnet"
```

## Security Monitoring

### SIEM Integration

Forward audit logs to your SIEM system:

1. **Splunk:**
   ```bash
   # Forward Vault audit logs
   [monitor:///var/log/vault/audit.log]
   sourcetype = vault:audit
   index = security
   ```

2. **Elastic Security:**
   ```yaml
   # Filebeat configuration
   filebeat.inputs:
     - type: log
       paths:
         - /var/log/omikuji/audit.log
       fields:
         app: omikuji
         type: audit
   ```

### Anomaly Detection

Watch for:
- Key access outside business hours
- Access from unusual IP addresses
- Rapid successive key retrievals
- Failed authentication spikes

## Incident Response

### Key Compromise Response

1. **Immediate Actions:**
   ```bash
   # Rotate compromised key
   omikuji key remove --network affected-network
   omikuji key import --network affected-network
   
   # Update monitoring
   # Add temporary elevated monitoring for the affected network
   ```

2. **Investigation:**
   - Review audit logs for unauthorized access
   - Check for any transactions from the compromised key
   - Identify the compromise vector

3. **Prevention:**
   - Implement additional access controls
   - Review and update security policies
   - Consider implementing key rotation schedule

### Backend Failure Response

1. **Vault Failure:**
   - Check Vault server health
   - Verify network connectivity
   - Review Vault audit logs
   - Consider failover to backup Vault cluster

2. **AWS Failure:**
   - Check AWS service health dashboard
   - Verify IAM permissions
   - Review CloudTrail logs
   - Consider multi-region setup

## Reporting

### Weekly Security Report

Generate automated reports including:
- Total key operations by type
- Failed operation counts
- Cache performance statistics
- Any security alerts triggered
- Backend availability percentage

### Monthly Audit Report

- All key lifecycle events (create/update/delete)
- Access patterns by network
- Performance trends
- Cost analysis (for AWS)
- Recommendations for optimization