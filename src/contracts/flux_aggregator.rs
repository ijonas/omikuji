use ethers::prelude::*;

// We use ethers-rs abigen macro to create Rust bindings from Solidity ABI
// This generates Rust code for interacting with the FluxAggregator contract
abigen!(
    IFluxAggregator,
    r#"[
        function latestAnswer() external view returns (int256)
        function latestTimestamp() external view returns (uint256)
        function latestRound() external view returns (uint256)
        function getAnswer(uint256 _roundId) external view returns (int256)
        function getTimestamp(uint256 _roundId) external view returns (uint256)
        function decimals() external view returns (uint8)
        function description() external view returns (string memory)
        function version() external view returns (uint256)
        function getRoundData(uint80 _roundId) returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
        function latestRoundData() returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
        function submit(uint256 _roundId, int256 _submission) external
        function oracleRoundState(address _oracle, uint32 _queriedRoundId) external view returns (bool _eligibleToSubmit, uint32 _roundId, int256 _latestSubmission, uint64 _startedAt, uint64 _timeout, uint128 _availableFunds, uint8 _oracleCount, uint128 _paymentAmount)
    ]"#,
);

/// Additional helper methods when using IFluxAggregator with a signer
#[allow(dead_code)]
impl<M: Middleware> IFluxAggregator<M> {
    /// Submits a new price to the FluxAggregator contract
    ///
    /// # Arguments
    /// * `round_id` - The round ID to submit the price for
    /// * `price` - The price to submit
    /// * `gas_limit` - Optional gas limit for the transaction
    ///
    /// # Returns
    /// The pending transaction
    pub async fn submit_price(
        &self,
        round_id: U256,
        price: I256,
        gas_limit: Option<U256>,
    ) -> Result<TransactionReceipt, ContractError<M>> {
        // Submit the transaction and wait for confirmation
        let submit = self.submit(round_id, price);

        // Add gas price and optional gas limit
        let tx = if let Some(limit) = gas_limit {
            submit.gas(limit)
        } else {
            submit
        };

        // Send and wait for confirmation
        tx.legacy()
          .gas_price(20_000_000_000u64)
          .send()
          .await?
          .await?
          .ok_or_else(|| ContractError::from(ProviderError::CustomError("Transaction dropped from mempool".into())))
    }

    /// Gets the current state of a round for the given oracle
    ///
    /// # Arguments
    /// * `oracle` - The address of the oracle
    /// * `round_id` - The round ID to query
    ///
    /// # Returns
    /// The round state information
    pub async fn get_round_state(
        &self,
        oracle: Address,
        round_id: u32,
    ) -> Result<(
        bool,        // eligibleToSubmit
        u32,         // roundId
        I256,        // latestSubmission
        u64,         // startedAt
        u64,         // timeout
        u128,        // availableFunds
        u8,          // oracleCount
        u128,        // paymentAmount
    ), ContractError<M>> {
        self.oracle_round_state(oracle, round_id).call().await
    }
}