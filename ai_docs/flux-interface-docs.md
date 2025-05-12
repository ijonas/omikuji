# FluxAggregator Interface Documentation

## Overview

The IFluxAggregator interface defines the methods available for interacting with the Chainlink FluxAggregator contract. This contract handles aggregating data pushed in from off-chain oracles and manages payments for their submissions.

## Interface Contract

```solidity
// SPDX-License-Identifier: MIT
pragma solidity 0.6.6;

import "@chainlink/contracts/src/v0.6/interfaces/AggregatorV2V3Interface.sol";

interface IFluxAggregator is AggregatorV2V3Interface {
    
    // Events
    event AvailableFundsUpdated(uint256 indexed amount);
    event RoundDetailsUpdated(
        uint128 indexed paymentAmount,
        uint32 indexed minSubmissionCount,
        uint32 indexed maxSubmissionCount,
        uint32 restartDelay,
        uint32 timeout
    );
    event OraclePermissionsUpdated(address indexed oracle, bool indexed whitelisted);
    event OracleAdminUpdated(address indexed oracle, address indexed newAdmin);
    event OracleAdminUpdateRequested(
        address indexed oracle,
        address admin,
        address newAdmin
    );
    event SubmissionReceived(
        int256 indexed submission,
        uint32 indexed round,
        address indexed oracle
    );
    event RequesterPermissionsSet(
        address indexed requester,
        bool authorized,
        uint32 delay
    );
    event ValidatorUpdated(address indexed previous, address indexed current);
    
    // Core Functions
    function submit(uint256 _roundId, int256 _submission) external;
    function oracleCount() external view returns (uint8);
    function requestNewRound() external returns (uint80);
    
    // Configuration Functions
    function changeOracles(
        address[] calldata _removed,
        address[] calldata _added,
        address[] calldata _addedAdmins,
        uint32 _minSubmissions,
        uint32 _maxSubmissions,
        uint32 _restartDelay
    ) external;
    
    function updateFutureRounds(
        uint128 _paymentAmount,
        uint32 _minSubmissions,
        uint32 _maxSubmissions,
        uint32 _restartDelay,
        uint32 _timeout
    ) external;
    
    function setRequesterPermissions(
        address _requester,
        bool _authorized,
        uint32 _delay
    ) external;
    
    function setValidator(address _newValidator) external;
    
    // Fund Management Functions
    function allocatedFunds() external view returns (uint128);
    function availableFunds() external view returns (uint128);
    function updateAvailableFunds() external;
    function withdrawablePayment(address _oracle) external view returns (uint256);
    function withdrawPayment(
        address _oracle,
        address _recipient,
        uint256 _amount
    ) external;
    function withdrawFunds(address _recipient, uint256 _amount) external;
    
    // Oracle Management Functions
    function getOracles() external view returns (address[] memory);
    function getAdmin(address _oracle) external view returns (address);
    function transferAdmin(address _oracle, address _newAdmin) external;
    function acceptAdmin(address _oracle) external;
    
    // Information Query Functions
    function oracleRoundState(
        address _oracle,
        uint32 _queriedRoundId
    ) external view returns (
        bool _eligibleToSubmit,
        uint32 _roundId,
        int256 _latestSubmission,
        uint64 _startedAt,
        uint64 _timeout,
        uint128 _availableFunds,
        uint8 _oracleCount,
        uint128 _paymentAmount
    );
    
    // Contract Parameters
    function paymentAmount() external view returns (uint128);
    function maxSubmissionCount() external view returns (uint32);
    function minSubmissionCount() external view returns (uint32);
    function restartDelay() external view returns (uint32);
    function timeout() external view returns (uint32);
    function decimals() external view returns (uint8);
    function description() external view returns (string memory);
    function version() external view returns (uint256);
    function linkToken() external view returns (address);
    function validator() external view returns (address);
}
```

## Function Documentation

### Oracle Submission

#### `submit(uint256 _roundId, int256 _submission)`
Allows oracles to submit new data for a specific round.

**Parameters:**
- `_roundId`: The ID of the round this submission pertains to
- `_submission`: The updated data that the oracle is submitting

**Requirements:**
- Caller must be an authorized oracle
- Submission must be within min/max value bounds
- Round must be accepting submissions

---

### Oracle Management

#### `changeOracles(address[] _removed, address[] _added, address[] _addedAdmins, uint32 _minSubmissions, uint32 _maxSubmissions, uint32 _restartDelay)`
Adds or removes oracles and updates round parameters.

**Parameters:**
- `_removed`: Array of oracle addresses to remove
- `_added`: Array of new oracle addresses to add
- `_addedAdmins`: Admin addresses for the new oracles
- `_minSubmissions`: New minimum submission count for each round
- `_maxSubmissions`: New maximum submission count for each round
- `_restartDelay`: Number of rounds an oracle must wait before initiating a round

**Requirements:**
- Only callable by owner
- Added oracles must have corresponding admin addresses
- Total oracles cannot exceed MAX_ORACLE_COUNT (77)

---

#### `getOracles() returns (address[])`
Returns the array of all oracle addresses.

**Returns:**
- Array of oracle addresses currently registered

---

#### `oracleCount() returns (uint8)`
Returns the total number of registered oracles.

**Returns:**
- Number of active oracles

---

### Fund Management

#### `allocatedFunds() returns (uint128)`
Returns the amount of LINK tokens allocated for oracle payments.

**Returns:**
- Amount of LINK allocated but not yet withdrawn

---

#### `availableFunds() returns (uint128)`
Returns the amount of LINK tokens available for future oracle payments.

**Returns:**
- Amount of LINK available for payments

---

#### `updateAvailableFunds()`
Recalculates the available LINK balance based on the contract's balance.

---

#### `withdrawablePayment(address _oracle) returns (uint256)`
Returns the amount of LINK an oracle can withdraw.

**Parameters:**
- `_oracle`: The oracle address to check

**Returns:**
- Amount of LINK available for withdrawal

---

#### `withdrawPayment(address _oracle, address _recipient, uint256 _amount)`
Transfers an oracle's earned LINK to a specified address.

**Parameters:**
- `_oracle`: The oracle whose funds are being withdrawn
- `_recipient`: The address to receive the LINK
- `_amount`: The amount of LINK to transfer

**Requirements:**
- Only callable by the oracle's admin
- Oracle must have sufficient withdrawable funds

---

#### `withdrawFunds(address _recipient, uint256 _amount)`
Allows the owner to withdraw excess LINK from the contract.

**Parameters:**
- `_recipient`: The address to receive the LINK
- `_amount`: The amount of LINK to withdraw

**Requirements:**
- Only callable by owner
- Must maintain required reserve funds

---

### Round Management

#### `requestNewRound() returns (uint80)`
Allows authorized requesters to initiate a new round.

**Returns:**
- The ID of the newly created round

**Requirements:**
- Caller must be an authorized requester
- Previous round must be completable or timed out

---

#### `updateFutureRounds(uint128 _paymentAmount, uint32 _minSubmissions, uint32 _maxSubmissions, uint32 _restartDelay, uint32 _timeout)`
Updates parameters for future rounds.

**Parameters:**
- `_paymentAmount`: Payment amount per oracle submission
- `_minSubmissions`: Minimum submissions required
- `_maxSubmissions`: Maximum submissions allowed
- `_restartDelay`: Rounds to wait before oracle can start new round
- `_timeout`: Time allowed for round completion

**Requirements:**
- Only callable by owner
- Max submissions must be >= min submissions
- Sufficient funds must be available

---

### Access Control

#### `getAdmin(address _oracle) returns (address)`
Returns the admin address for a specific oracle.

**Parameters:**
- `_oracle`: The oracle address to query

**Returns:**
- The admin address for the oracle

---

#### `transferAdmin(address _oracle, address _newAdmin)`
Initiates a transfer of admin rights for an oracle.

**Parameters:**
- `_oracle`: The oracle whose admin is being transferred
- `_newAdmin`: The new admin address

**Requirements:**
- Only callable by current admin

---

#### `acceptAdmin(address _oracle)`
Accepts the admin role transfer for an oracle.

**Parameters:**
- `_oracle`: The oracle whose admin role is being accepted

**Requirements:**
- Only callable by pending admin

---

#### `setRequesterPermissions(address _requester, bool _authorized, uint32 _delay)`
Sets permissions for non-oracle addresses to request new rounds.

**Parameters:**
- `_requester`: Address to set permissions for
- `_authorized`: Whether the address can request rounds
- `_delay`: Number of rounds to wait between requests

**Requirements:**
- Only callable by owner

---

### Validation

#### `setValidator(address _newValidator)`
Updates the address for external data validation.

**Parameters:**
- `_newValidator`: Address of the new validation contract

**Requirements:**
- Only callable by owner

---

### Query Functions

#### `oracleRoundState(address _oracle, uint32 _queriedRoundId)`
Provides comprehensive state information for an oracle in a specific round.

**Parameters:**
- `_oracle`: The oracle address to query
- `_queriedRoundId`: The round ID to query (0 for suggested round)

**Returns:**
- `_eligibleToSubmit`: Whether oracle can submit to this round
- `_roundId`: The round ID
- `_latestSubmission`: Oracle's latest submission value
- `_startedAt`: Round start timestamp
- `_timeout`: Round timeout period
- `_availableFunds`: Available LINK in contract
- `_oracleCount`: Total number of oracles
- `_paymentAmount`: Payment per submission

**Requirements:**
- Only callable off-chain (tx.origin == msg.sender)

---

### Configuration Parameters

#### `paymentAmount() returns (uint128)`
Returns the payment amount per oracle submission.

#### `maxSubmissionCount() returns (uint32)`
Returns the maximum number of submissions per round.

#### `minSubmissionCount() returns (uint32)`
Returns the minimum number of submissions required per round.

#### `restartDelay() returns (uint32)`
Returns the number of rounds an oracle must wait before starting a new round.

#### `timeout() returns (uint32)`
Returns the timeout period for rounds in seconds.

#### `decimals() returns (uint8)`
Returns the number of decimals for the aggregated answer.

#### `description() returns (string)`
Returns a description of what the aggregator is reporting.

#### `version() returns (uint256)`
Returns the version number of the aggregator.

### Contract References

#### `linkToken() returns (address)`
Returns the address of the LINK token contract.

#### `validator() returns (address)`
Returns the address of the validation contract.

---

## Inherited Functions from AggregatorV2V3Interface

The contract also inherits all functions from AggregatorV2V3Interface, including:

- `latestAnswer() returns (int256)`
- `latestTimestamp() returns (uint256)`
- `latestRound() returns (uint256)`
- `getAnswer(uint256 _roundId) returns (int256)`
- `getTimestamp(uint256 _roundId) returns (uint256)`
- `getRoundData(uint80 _roundId) returns (uint80, int256, uint256, uint256, uint80)`
- `latestRoundData() returns (uint80, int256, uint256, uint256, uint80)`

These functions provide access to aggregated price data and round information.