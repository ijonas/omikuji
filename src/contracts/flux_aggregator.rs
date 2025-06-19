use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::utils::parse_units;
use crate::gas::GasEstimate;
use crate::config::models::{Network, GasConfig, FeeBumpingConfig};
use tracing::{info, warn, error};
use tokio::time::{sleep, Duration};

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
        function minSubmissionValue() external view returns (int256)
        function maxSubmissionValue() external view returns (int256)
        function getRoundData(uint80 _roundId) returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
        function latestRoundData() returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
        function submit(uint256 _roundId, int256 _submission) external
        function oracleRoundState(address _oracle, uint32 _queriedRoundId) external view returns (bool _eligibleToSubmit, uint32 _roundId, int256 _latestSubmission, uint64 _startedAt, uint64 _timeout, uint128 _availableFunds, uint8 _oracleCount, uint128 _paymentAmount)
    ]"#,
);

/// Additional helper methods when using IFluxAggregator with a signer
#[allow(dead_code)]
impl<M: Middleware> IFluxAggregator<M> {
    /// Submits a new price to the FluxAggregator contract with gas estimation
    ///
    /// # Arguments
    /// * `round_id` - The round ID to submit the price for
    /// * `price` - The price to submit
    /// * `network_config` - Network configuration for gas settings
    ///
    /// # Returns
    /// The transaction receipt
    pub async fn submit_price_with_gas_estimation(
        &self,
        round_id: U256,
        price: I256,
        network_config: &Network,
    ) -> Result<TransactionReceipt, ContractError<M>> {
        // Create the base transaction call
        let contract_call = self.submit(round_id, price);
        
        // Build the transaction request
        let tx_request = contract_call.tx;
        let mut typed_tx = TypedTransaction::Legacy(tx_request.clone().into());
        
        // Estimate gas using the middleware's provider
        let gas_estimate = self.estimate_gas_for_tx(&typed_tx, network_config).await?;
        
        // Apply gas settings
        typed_tx.set_gas(gas_estimate.gas_limit);
        
        // Apply fee settings based on transaction type
        match network_config.transaction_type.to_lowercase().as_str() {
            "legacy" => {
                if let Some(gas_price) = gas_estimate.gas_price {
                    typed_tx.set_gas_price(gas_price);
                }
            }
            "eip1559" => {
                if let (Some(max_fee), Some(priority_fee)) = 
                    (gas_estimate.max_fee_per_gas, gas_estimate.max_priority_fee_per_gas) {
                    // Convert to EIP-1559 transaction
                    let legacy_tx = match &typed_tx {
                        TypedTransaction::Legacy(tx) => tx.clone(),
                        _ => return Err(ContractError::from(
                            ProviderError::CustomError("Unexpected transaction type".into())
                        )),
                    };
                    
                    let eip1559_tx = Eip1559TransactionRequest {
                        from: legacy_tx.from,
                        to: legacy_tx.to,
                        gas: Some(gas_estimate.gas_limit),
                        value: legacy_tx.value,
                        data: legacy_tx.data,
                        nonce: legacy_tx.nonce,
                        access_list: Default::default(),
                        max_priority_fee_per_gas: Some(priority_fee),
                        max_fee_per_gas: Some(max_fee),
                        chain_id: legacy_tx.chain_id,
                    };
                    
                    typed_tx = TypedTransaction::Eip1559(eip1559_tx);
                }
            }
            _ => {}
        }

        // Send transaction with retry logic
        self.send_with_retry(typed_tx, &gas_estimate, &network_config.gas_config).await
    }

    /// Legacy submit_price method for backwards compatibility
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

    /// Estimate gas for a transaction
    async fn estimate_gas_for_tx(
        &self,
        tx: &TypedTransaction,
        network_config: &Network,
    ) -> Result<GasEstimate, ContractError<M>> {
        let gas_config = &network_config.gas_config;
        
        // Estimate gas limit
        let gas_limit = if let Some(manual_limit) = gas_config.gas_limit {
            U256::from(manual_limit)
        } else {
            match self.client().estimate_gas(tx, None).await {
                Ok(estimated) => {
                    let multiplier = gas_config.gas_multiplier;
                    estimated.saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000)
                }
                Err(_) => U256::from(200_000) // Fallback
            }
        };

        // Estimate fees based on transaction type
        match network_config.transaction_type.to_lowercase().as_str() {
            "legacy" => {
                let gas_price = if let Some(manual_price) = gas_config.gas_price_gwei {
                    parse_units(manual_price, "gwei").unwrap().into()
                } else {
                    match self.client().get_gas_price().await {
                        Ok(price) => {
                            let multiplier = gas_config.gas_multiplier;
                            price.saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000)
                        }
                        Err(_) => parse_units(20, "gwei").unwrap().into() // Fallback
                    }
                };
                
                Ok(GasEstimate {
                    gas_limit,
                    gas_price: Some(gas_price),
                    max_fee_per_gas: None,
                    max_priority_fee_per_gas: None,
                })
            }
            "eip1559" => {
                let (max_fee, priority_fee) = if let (Some(max_f), Some(pri_f)) = 
                    (gas_config.max_fee_per_gas_gwei, gas_config.max_priority_fee_per_gas_gwei) {
                    (
                        parse_units(max_f, "gwei").unwrap().into(),
                        parse_units(pri_f, "gwei").unwrap().into()
                    )
                } else {
                    match self.client().get_gas_price().await {
                        Ok(gas_price) => {
                            let multiplier = gas_config.gas_multiplier;
                            let priority_fee = parse_units(2, "gwei").unwrap().into();
                            let max_fee = gas_price.saturating_add(priority_fee)
                                .saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000);
                            (max_fee, priority_fee)
                        }
                        Err(_) => (
                            parse_units(50, "gwei").unwrap().into(),
                            parse_units(2, "gwei").unwrap().into()
                        ) // Fallback
                    }
                };
                
                Ok(GasEstimate {
                    gas_limit,
                    gas_price: None,
                    max_fee_per_gas: Some(max_fee),
                    max_priority_fee_per_gas: Some(priority_fee),
                })
            }
            _ => Err(ContractError::from(
                ProviderError::CustomError("Invalid transaction type".into())
            ))
        }
    }

    /// Send transaction with retry logic for fee bumping
    async fn send_with_retry(
        &self,
        mut tx: TypedTransaction,
        original_estimate: &GasEstimate,
        gas_config: &GasConfig,
    ) -> Result<TransactionReceipt, ContractError<M>> {
        let mut retry_count = 0;
        let mut current_estimate = original_estimate.clone();

        loop {
            info!("Sending transaction (attempt {})", retry_count + 1);
            
            // Send the transaction
            let client = self.client();
            let pending_tx = match client.send_transaction(tx.clone(), None).await {
                Ok(tx) => tx,
                Err(e) => {
                    error!("Failed to send transaction: {}", e);
                    return Err(ContractError::from(ProviderError::CustomError(e.to_string())));
                }
            };
            let tx_hash = pending_tx.tx_hash();
            info!("Transaction sent: {:?}", tx_hash);

            // Wait for confirmation with timeout
            let wait_duration = if retry_count == 0 {
                Duration::from_secs(gas_config.fee_bumping.initial_wait_seconds)
            } else {
                Duration::from_secs(gas_config.fee_bumping.initial_wait_seconds * 2)
            };

            match tokio::time::timeout(wait_duration, pending_tx).await {
                Ok(Ok(Some(receipt))) => {
                    info!("Transaction confirmed: {:?}, gas used: {:?}", 
                        receipt.transaction_hash, receipt.gas_used);
                    return Ok(receipt);
                }
                Ok(Ok(None)) => {
                    warn!("Transaction dropped from mempool: {:?}", tx_hash);
                }
                Ok(Err(e)) => {
                    error!("Transaction failed: {:?}, error: {}", tx_hash, e);
                    return Err(ContractError::from(e));
                }
                Err(_) => {
                    warn!("Transaction timed out after {} seconds: {:?}", 
                        wait_duration.as_secs(), tx_hash);
                }
            }

            // Check if we should retry
            if !gas_config.fee_bumping.enabled || retry_count >= gas_config.fee_bumping.max_retries {
                return Err(ContractError::from(
                    ProviderError::CustomError("Transaction failed after max retries".into())
                ));
            }

            // Bump fees for retry
            retry_count += 1;
            current_estimate = self.bump_fees(&current_estimate, retry_count, &gas_config.fee_bumping);
            
            // Update transaction with new fees
            tx.set_gas(current_estimate.gas_limit);
            
            match tx {
                TypedTransaction::Legacy(ref mut legacy_tx) => {
                    if let Some(new_price) = current_estimate.gas_price {
                        legacy_tx.gas_price = Some(new_price);
                        info!("Bumped gas price to: {} gwei", 
                            ethers::utils::format_units(new_price, "gwei").unwrap_or_default());
                    }
                }
                TypedTransaction::Eip1559(ref mut eip1559_tx) => {
                    if let (Some(max_fee), Some(priority_fee)) = 
                        (current_estimate.max_fee_per_gas, current_estimate.max_priority_fee_per_gas) {
                        eip1559_tx.max_fee_per_gas = Some(max_fee);
                        eip1559_tx.max_priority_fee_per_gas = Some(priority_fee);
                        info!("Bumped EIP-1559 fees to: max_fee={} gwei, priority_fee={} gwei",
                            ethers::utils::format_units(max_fee, "gwei").unwrap_or_default(),
                            ethers::utils::format_units(priority_fee, "gwei").unwrap_or_default());
                    }
                }
                _ => {}
            }

            // Wait a bit before retrying
            sleep(Duration::from_secs(5)).await;
        }
    }

    /// Bump fees for a retry attempt
    fn bump_fees(&self, original: &GasEstimate, retry_count: u8, fee_bumping: &FeeBumpingConfig) -> GasEstimate {
        let bump_percent = fee_bumping.fee_increase_percent;
        let multiplier = 1.0 + (bump_percent / 100.0) * retry_count as f64;
        
        GasEstimate {
            gas_limit: original.gas_limit, // Keep same gas limit
            gas_price: original.gas_price.map(|p| {
                p.saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000)
            }),
            max_fee_per_gas: original.max_fee_per_gas.map(|p| {
                p.saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000)
            }),
            max_priority_fee_per_gas: original.max_priority_fee_per_gas.map(|p| {
                p.saturating_mul(U256::from((multiplier * 1000.0) as u64)) / U256::from(1000)
            }),
        }
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