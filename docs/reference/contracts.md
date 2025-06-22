# Smart Contract Reference

This document describes the smart contract interfaces supported by Omikuji and how to integrate with them.

## Supported Contract Types

### Chainlink FluxAggregator (`fluxmon`)

Currently, Omikuji supports Chainlink FluxAggregator contracts, which are widely used for price feeds on Ethereum networks.

## FluxAggregator Interface

The FluxAggregator contract provides a decentralized oracle solution for price feeds. Omikuji interacts with these contracts as an oracle node operator.

### Key Functions Used by Omikuji

#### `submit(uint256 _roundId, int256 _submission)`
Submits a new value to the aggregator.

- **Parameters**:
  - `_roundId`: The round ID to submit for
  - `_submission`: The value to submit (scaled by decimals)
- **Access**: Restricted to whitelisted oracle addresses

#### `latestRoundData()`
Returns the latest price data.

- **Returns**:
  - `roundId`: The round ID
  - `answer`: The price value
  - `startedAt`: Timestamp when round started
  - `updatedAt`: Timestamp of last update
  - `answeredInRound`: Round ID when answer was computed

#### `decimals()`
Returns the number of decimals used by the price feed.

- **Returns**: `uint8` decimal places

#### `description()`
Returns a human-readable description of the price feed.

- **Returns**: `string` description (e.g., "ETH / USD")

#### `minSubmissionValue()` and `maxSubmissionValue()`
Returns the acceptable range for submitted values.

- **Returns**: `int256` minimum/maximum allowed value

### Contract Configuration

Omikuji can either read configuration from the contract or use manual settings:

```yaml
# Option 1: Read from contract (recommended)
datafeeds:
  - name: eth_usd
    read_contract_config: true
    # Omikuji will call decimals(), minSubmissionValue(), maxSubmissionValue()

# Option 2: Manual configuration
datafeeds:
  - name: eth_usd
    read_contract_config: false
    decimals: 8              # Price has 8 decimal places
    min_value: 0            # Minimum price $0
    max_value: 1000000      # Maximum price $1,000,000
```

## Value Scaling

Price values are scaled according to the contract's decimals:

- **Decimals = 8**: Price of $1,234.56 is submitted as `123456000000`
- **Decimals = 18**: Price of $1,234.56 is submitted as `1234560000000000000000`

Formula: `submission_value = price * (10 ^ decimals)`

## Round Management

FluxAggregator uses a round-based system:

1. **Round ID**: Incremental identifier for each price update round
2. **Oracle Participation**: Multiple oracles submit to the same round
3. **Aggregation**: Contract aggregates submissions to determine final price

Omikuji handles round management automatically:
- Queries current round via `latestRoundData()`
- Submits to the appropriate round
- Handles round transitions

## Access Control

### Oracle Whitelisting

FluxAggregator contracts restrict submission access:

1. **Check Access**: Ensure your wallet address is whitelisted
2. **Request Access**: Contact the contract owner for oracle privileges
3. **Verify**: Test submission capability before production use

### Common Errors

- `"No access"`: Wallet not whitelisted as oracle
- `"Invalid round"`: Attempting to submit to wrong round
- `"Value below minimum"`: Submission below `minSubmissionValue`
- `"Value above maximum"`: Submission above `maxSubmissionValue`

## Contract Deployment

While Omikuji doesn't deploy contracts, here's what you need:

### For Testing

Use existing test contracts or deploy your own:

```solidity
// Example FluxAggregator deployment parameters
contract = new FluxAggregator(
    _link,           // LINK token address
    _paymentAmount,  // Payment per submission
    _timeout,        // Seconds before round times out
    _validator,      // Validator contract address
    _minSubmissionValue,
    _maxSubmissionValue,
    _decimals,
    _description
);
```

### For Production

1. Use official Chainlink price feeds when available
2. Deploy custom FluxAggregator for proprietary data
3. Ensure proper access control and validation

## Integration Examples

### Basic Integration

```yaml
datafeeds:
  - name: eth_usd
    networks: ethereum
    contract_address: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"
    contract_type: fluxmon
    read_contract_config: true
    # ... other config
```

### Custom Decimals

For non-standard decimal places:

```yaml
datafeeds:
  - name: custom_feed
    contract_type: fluxmon
    read_contract_config: false
    decimals: 6  # USDC-style decimals
    # ... other config
```

### Multiple Networks

Same feed on different networks:

```yaml
datafeeds:
  - name: eth_usd_mainnet
    networks: ethereum
    contract_address: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"
    # ... config
    
  - name: eth_usd_polygon
    networks: polygon
    contract_address: "0xF9680D99D6C9589e2a93a78A04A279e509205945"
    # ... config
```

## Best Practices

1. **Always Verify Contract**
   - Check contract is verified on Etherscan
   - Confirm it implements FluxAggregator interface
   - Test in testnet first

2. **Monitor Bounds**
   - Respect min/max submission values
   - Log warnings when approaching limits
   - Have alerts for contract changes

3. **Handle Failures**
   - Implement retry logic for transient failures
   - Alert on repeated submission failures
   - Monitor gas usage and costs

4. **Security**
   - Keep oracle private keys secure
   - Monitor for unauthorized contract changes
   - Regular security audits

## Future Contract Support

Planned support for additional contract types:

- Chainlink OCR (Off-Chain Reporting)
- Chainlink Data Streams
- Custom oracle interfaces

## Troubleshooting

### "Transaction reverted"
1. Check wallet is whitelisted as oracle
2. Verify submission value is within bounds
3. Ensure sufficient gas provided
4. Check round ID is current

### "Cannot read contract config"
1. Verify contract implements expected interface
2. Check contract address is correct
3. Ensure RPC connection is working
4. Try manual configuration as fallback

### Testing Contracts

For local development:
1. Deploy FluxAggregator to local network
2. Whitelist your test wallet
3. Use manual configuration to skip contract reads
4. Monitor contract events for debugging

## Additional Resources

- [Chainlink Documentation](https://docs.chain.link/)
- [FluxAggregator Source Code](https://github.com/smartcontractkit/chainlink/blob/develop/contracts/src/v0.6/FluxAggregator.sol)
- [Omikuji Contract Examples](https://github.com/ijonas/omikuji/tree/main/examples/contracts)