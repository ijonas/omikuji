# Scheduled Tasks Guide

This guide explains how to configure and use scheduled tasks in Omikuji to automatically execute smart contract functions on a time-based schedule.

## Overview

Scheduled tasks allow Omikuji to:
- Execute smart contract functions on a cron-based schedule
- Check conditions before execution to avoid unnecessary transactions
- Support complex parameter encoding for function calls
- Track gas usage and execution history
- Handle errors gracefully with detailed logging

## Configuration

Add a `scheduled_tasks` section to your Omikuji configuration file:

```yaml
scheduled_tasks:
  - name: "task_name"
    network: "network_name"
    schedule: "cron_expression"
    check_condition: # Optional
      contract_address: "0x..."
      property: "propertyName"  # OR function: "functionName()"
      expected_value: true
    target_function:
      contract_address: "0x..."
      function: "functionName(param1Type,param2Type)"
      parameters:
        - value: value1
          type: "param1Type"
        - value: value2
          type: "param2Type"
    gas_config: # Optional
      max_gas_price_gwei: 100
      gas_limit: 500000
      priority_fee_gwei: 2
```

## Cron Schedule Format

Scheduled tasks use standard cron expressions:

```
┌───────────── minute (0 - 59)
│ ┌───────────── hour (0 - 23)
│ │ ┌───────────── day of the month (1 - 31)
│ │ │ ┌───────────── month (1 - 12)
│ │ │ │ ┌───────────── day of the week (0 - 6) (Sunday to Saturday)
│ │ │ │ │
* * * * *
```

Common examples:
- `*/5 * * * *` - Every 5 minutes
- `0 * * * *` - Every hour
- `0 0 * * *` - Daily at midnight
- `0 0 * * 0` - Weekly on Sunday
- `0 0 1 * *` - Monthly on the 1st

## Condition Checking

Before executing the target function, Omikuji can check a condition:

### Boolean Property Check
```yaml
check_condition:
  contract_address: "0x1234..."
  property: "isReady"  # Must be a public boolean property
  expected_value: true
```

### View Function Check
```yaml
check_condition:
  contract_address: "0x1234..."
  function: "canExecute()"  # Must be parameterless and return bool
  expected_value: true
```

If the condition is not met, the task execution is skipped until the next scheduled time.

## Function Parameters

### Supported Types

Currently supported parameter types:
- `uint256` - Unsigned integers
- `address` - Ethereum addresses
- `bool` - Boolean values
- `address[]` - Array of addresses

### Examples

#### No Parameters
```yaml
target_function:
  contract_address: "0x1234..."
  function: "execute()"
  parameters: []
```

#### Single Parameter
```yaml
target_function:
  contract_address: "0x1234..."
  function: "setValue(uint256)"
  parameters:
    - value: 12345
      type: "uint256"
```

#### Multiple Parameters
```yaml
target_function:
  contract_address: "0x1234..."
  function: "transfer(address,uint256)"
  parameters:
    - value: "0xRecipientAddress"
      type: "address"
    - value: "1000000000000000000"  # 1 ETH in wei
      type: "uint256"
```

#### Array Parameters
```yaml
target_function:
  contract_address: "0x1234..."
  function: "batchTransfer(address[])"
  parameters:
    - value: ["0xAddr1", "0xAddr2", "0xAddr3"]
      type: "address[]"
```

## Gas Configuration

You can specify gas settings for each task:

```yaml
gas_config:
  max_gas_price_gwei: 100      # Maximum gas price in Gwei
  gas_limit: 500000            # Gas limit for the transaction
  priority_fee_gwei: 2         # EIP-1559 priority fee in Gwei
```

If not specified, Omikuji will use automatic gas estimation.

## Example Use Cases

### 1. Daily Reward Distribution

```yaml
scheduled_tasks:
  - name: "daily_rewards"
    network: "ethereum-mainnet"
    schedule: "0 0 * * *"  # Daily at midnight
    check_condition:
      contract_address: "0xRewardContract"
      property: "rewardsAvailable"
      expected_value: true
    target_function:
      contract_address: "0xRewardContract"
      function: "distributeRewards()"
      parameters: []
```

### 2. Hourly Price Oracle Update

```yaml
scheduled_tasks:
  - name: "price_update"
    network: "polygon"
    schedule: "0 * * * *"  # Every hour
    check_condition:
      contract_address: "0xOracle"
      function: "needsUpdate()"
      expected_value: true
    target_function:
      contract_address: "0xOracle"
      function: "updatePrice(uint256)"
      parameters:
        - value: 1234567890
          type: "uint256"
```

### 3. Weekly Governance Execution

```yaml
scheduled_tasks:
  - name: "governance_execution"
    network: "ethereum-mainnet"
    schedule: "0 0 * * 0"  # Weekly on Sunday
    check_condition:
      contract_address: "0xGovernance"
      function: "hasExecutableProposals()"
      expected_value: true
    target_function:
      contract_address: "0xGovernance"
      function: "executeProposals(uint256)"
      parameters:
        - value: 10  # Execute up to 10 proposals
          type: "uint256"
```

## Monitoring and Debugging

### Logs

Scheduled task execution is logged with detailed information:
- Task execution start/completion
- Condition check results
- Transaction hashes
- Gas usage
- Error messages

### Metrics

Prometheus metrics are available for monitoring:
- `omikuji_scheduled_task_executions_total` - Total executions by task and status
- `omikuji_scheduled_task_last_execution_timestamp` - Last execution time
- `omikuji_scheduled_task_gas_used_total` - Gas consumption tracking

### Database Logging

If a database is configured, execution history is stored in the `scheduled_execution_log` table for analysis.

## Best Practices

1. **Test First**: Always test your scheduled tasks on a testnet before mainnet deployment
2. **Gas Limits**: Set reasonable gas limits to prevent excessive costs
3. **Condition Checks**: Use condition checks to avoid unnecessary transactions
4. **Error Handling**: Monitor logs for failed executions and adjust accordingly
5. **Schedule Timing**: Consider network congestion when scheduling tasks

## Security Considerations

- Ensure contract addresses are correct and verified
- Use condition checks to prevent execution when not needed
- Set gas limits to prevent excessive spending
- Monitor execution logs for anomalies
- Keep private keys secure using Omikuji's key storage options

## Troubleshooting

### Task Not Executing

1. Check logs for error messages
2. Verify the cron expression is correct
3. Ensure the network has a loaded wallet
4. Confirm contract addresses are valid
5. Test condition checks manually

### Transaction Failures

1. Check gas configuration
2. Verify function signature matches contract ABI
3. Ensure parameters are correctly formatted
4. Check wallet balance for gas
5. Review contract requirements

### Condition Always False

1. Manually call the condition function/property
2. Verify expected value matches actual return
3. Check contract state
4. Ensure property/function is public view