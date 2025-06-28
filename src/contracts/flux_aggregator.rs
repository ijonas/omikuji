use crate::config::models::Network as NetworkConfig;
use crate::database::TransactionLogRepository;
use crate::gas::GasEstimate;
use crate::metrics::gas_metrics::{GasMetrics, TransactionDetails};
use crate::metrics::ContractMetrics;
use alloy::{
    network::{Ethereum, TransactionBuilder},
    primitives::{Address, I256, U256},
    providers::Provider,
    rpc::types::{BlockId, TransactionReceipt, TransactionRequest},
    sol,
    sol_types::SolCall,
    transports::Transport,
};
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::Duration;
use tracing::{error, info, warn};

// Define the Solidity interface using alloy's sol! macro
sol! {
    #[sol(rpc)]
    interface IFluxAggregator {
        function latestAnswer() external view returns (int256);
        function latestTimestamp() external view returns (uint256);
        function latestRound() external view returns (uint256);
        function getAnswer(uint256 _roundId) external view returns (int256);
        function getTimestamp(uint256 _roundId) external view returns (uint256);
        function decimals() external view returns (uint8);
        function description() external view returns (string memory);
        function version() external view returns (uint256);
        function minSubmissionValue() external view returns (int256);
        function maxSubmissionValue() external view returns (int256);
        function getRoundData(uint80 _roundId) external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
        function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
        function submit(uint256 _roundId, int256 _submission) external;
        function oracleRoundState(address _oracle, uint32 _queriedRoundId) external view returns (bool _eligibleToSubmit, uint32 _roundId, int256 _latestSubmission, uint64 _startedAt, uint64 _timeout, uint128 _availableFunds, uint8 _oracleCount, uint128 _paymentAmount);
    }
}

/// Wrapper for FluxAggregator contract interactions
pub struct FluxAggregatorContract<T: Transport + Clone, P: Provider<T, Ethereum>> {
    address: Address,
    provider: P,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Transport + Clone, P: Provider<T, Ethereum> + Clone> FluxAggregatorContract<T, P> {
    /// Create a new FluxAggregator contract instance
    pub fn new(address: Address, provider: P) -> Self {
        Self {
            address,
            provider,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the latest answer from the contract
    pub async fn latest_answer(&self) -> Result<I256> {
        self.latest_answer_with_metrics(None, None).await
    }

    /// Get the latest answer from the contract with metrics
    pub async fn latest_answer_with_metrics(
        &self,
        feed_name: Option<&str>,
        network: Option<&str>,
    ) -> Result<I256> {
        let start = Instant::now();
        let call = IFluxAggregator::latestAnswerCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());

        match self.provider.call(&tx).block(BlockId::latest()).await {
            Ok(result) => {
                let duration = start.elapsed();

                // Record metrics if context is provided
                if let (Some(feed), Some(net)) = (feed_name, network) {
                    ContractMetrics::record_contract_read(
                        feed,
                        net,
                        "latestAnswer",
                        true,
                        duration,
                        None,
                    );
                }

                let decoded = IFluxAggregator::latestAnswerCall::abi_decode_returns(&result, true)?;
                Ok(decoded._0)
            }
            Err(e) => {
                let duration = start.elapsed();

                // Record metrics if context is provided
                if let (Some(feed), Some(net)) = (feed_name, network) {
                    ContractMetrics::record_contract_read(
                        feed,
                        net,
                        "latestAnswer",
                        false,
                        duration,
                        Some(&e.to_string()),
                    );
                }

                Err(e.into())
            }
        }
    }

    /// Get the latest timestamp
    pub async fn latest_timestamp(&self) -> Result<U256> {
        self.latest_timestamp_with_metrics(None, None).await
    }

    /// Get the latest timestamp with metrics
    pub async fn latest_timestamp_with_metrics(
        &self,
        feed_name: Option<&str>,
        network: Option<&str>,
    ) -> Result<U256> {
        let start = Instant::now();
        let call = IFluxAggregator::latestTimestampCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());

        match self.provider.call(&tx).block(BlockId::latest()).await {
            Ok(result) => {
                let duration = start.elapsed();

                // Record metrics if context is provided
                if let (Some(feed), Some(net)) = (feed_name, network) {
                    ContractMetrics::record_contract_read(
                        feed,
                        net,
                        "latestTimestamp",
                        true,
                        duration,
                        None,
                    );
                }

                let decoded =
                    IFluxAggregator::latestTimestampCall::abi_decode_returns(&result, true)?;
                Ok(decoded._0)
            }
            Err(e) => {
                let duration = start.elapsed();

                // Record metrics if context is provided
                if let (Some(feed), Some(net)) = (feed_name, network) {
                    ContractMetrics::record_contract_read(
                        feed,
                        net,
                        "latestTimestamp",
                        false,
                        duration,
                        Some(&e.to_string()),
                    );
                }

                Err(e.into())
            }
        }
    }

    /// Get the latest round
    pub async fn latest_round(&self) -> Result<U256> {
        let call = IFluxAggregator::latestRoundCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::latestRoundCall::abi_decode_returns(&result, true)?;
        Ok(decoded._0)
    }

    /// Get decimals
    pub async fn decimals(&self) -> Result<u8> {
        let call = IFluxAggregator::decimalsCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::decimalsCall::abi_decode_returns(&result, true)?;
        Ok(decoded._0)
    }

    /// Get min submission value
    pub async fn min_submission_value(&self) -> Result<I256> {
        let call = IFluxAggregator::minSubmissionValueCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::minSubmissionValueCall::abi_decode_returns(&result, true)?;
        Ok(decoded._0)
    }

    /// Get max submission value
    pub async fn max_submission_value(&self) -> Result<I256> {
        let call = IFluxAggregator::maxSubmissionValueCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::maxSubmissionValueCall::abi_decode_returns(&result, true)?;
        Ok(decoded._0)
    }

    /// Get description
    #[allow(dead_code)]
    pub async fn description(&self) -> Result<String> {
        let call = IFluxAggregator::descriptionCall {};
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::descriptionCall::abi_decode_returns(&result, true)?;
        Ok(decoded._0)
    }

    /// Get oracle round state
    #[allow(dead_code)]
    pub async fn oracle_round_state(
        &self,
        oracle: Address,
        queried_round_id: u32,
    ) -> Result<IFluxAggregator::oracleRoundStateReturn> {
        let call = IFluxAggregator::oracleRoundStateCall {
            _oracle: oracle,
            _queriedRoundId: queried_round_id,
        };
        let tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());
        let result = self.provider.call(&tx).block(BlockId::latest()).await?;

        let decoded = IFluxAggregator::oracleRoundStateCall::abi_decode_returns(&result, true)?;
        Ok(decoded)
    }

    /// Submit a new price to the FluxAggregator contract with gas estimation and retry logic
    pub async fn submit_price_with_gas_estimation(
        &self,
        round_id: U256,
        price: I256,
        network_config: &NetworkConfig,
        feed_name: &str,
        tx_log_repo: Option<Arc<TransactionLogRepository>>,
        from_address: Option<Address>,
    ) -> Result<TransactionReceipt> {
        let gas_config = &network_config.gas_config;
        let fee_bumping = &gas_config.fee_bumping;

        // Create the function call
        let call = IFluxAggregator::submitCall {
            _roundId: round_id,
            _submission: price,
        };

        // Build base transaction request
        let mut tx = TransactionRequest::default()
            .to(self.address)
            .input(call.abi_encode().into());

        // Set from address if provided (needed for accurate gas estimation)
        if let Some(from) = from_address {
            tx = tx.from(from);
        }

        // Estimate gas
        let gas_estimator = crate::gas::GasEstimator::<T, P>::new(
            Arc::new(self.provider.clone()),
            network_config.clone(),
        );
        let mut gas_estimate = gas_estimator.estimate_gas(&tx).await?;

        let mut attempt = 0;
        let max_attempts = if fee_bumping.enabled {
            fee_bumping.max_retries + 1
        } else {
            1
        };

        loop {
            attempt += 1;

            // Apply gas settings
            tx = tx.with_gas_limit(gas_estimate.gas_limit.to::<u64>());

            // Apply fee settings based on transaction type
            match network_config.transaction_type.to_lowercase().as_str() {
                "legacy" => {
                    if let Some(gas_price) = gas_estimate.gas_price {
                        tx = tx.with_gas_price(gas_price.to::<u128>());
                    }
                }
                "eip1559" => {
                    if let Some(max_fee) = gas_estimate.max_fee_per_gas {
                        tx = tx.with_max_fee_per_gas(max_fee.to::<u128>());
                    }
                    if let Some(priority_fee) = gas_estimate.max_priority_fee_per_gas {
                        tx = tx.with_max_priority_fee_per_gas(priority_fee.to::<u128>());
                    }
                }
                _ => {
                    warn!("Unknown transaction type, defaulting to EIP-1559");
                    if let Some(max_fee) = gas_estimate.max_fee_per_gas {
                        tx = tx.with_max_fee_per_gas(max_fee.to::<u128>());
                    }
                    if let Some(priority_fee) = gas_estimate.max_priority_fee_per_gas {
                        tx = tx.with_max_priority_fee_per_gas(priority_fee.to::<u128>());
                    }
                }
            }

            info!("Sending transaction (attempt {})", attempt);

            // Record contract write attempt
            let write_start = Instant::now();

            // Send transaction
            let pending_tx = match self.provider.send_transaction(tx.clone()).await {
                Ok(tx) => tx,
                Err(e) => {
                    let write_duration = write_start.elapsed();
                    error!("Failed to send transaction: {}", e);

                    // Record failed contract write
                    ContractMetrics::record_contract_write(
                        feed_name,
                        &network_config.name,
                        false,
                        write_duration,
                        None,
                    );

                    // Check for specific error types
                    let error_str = e.to_string();
                    if error_str.contains("revert") {
                        ContractMetrics::record_transaction_revert(
                            feed_name,
                            &network_config.name,
                            &error_str,
                        );
                    }

                    if attempt >= max_attempts {
                        ContractMetrics::record_transaction_retry(
                            feed_name,
                            &network_config.name,
                            "max_attempts_reached",
                            attempt as u32,
                        );
                        return Err(anyhow::anyhow!(
                            "Failed to send transaction after {} attempts: {}",
                            attempt as u32,
                            e
                        ));
                    }

                    ContractMetrics::record_transaction_retry(
                        feed_name,
                        &network_config.name,
                        "send_error",
                        attempt as u32,
                    );
                    continue;
                }
            };

            let tx_hash = *pending_tx.tx_hash();
            info!("Transaction sent: 0x{:x}", tx_hash);

            // Wait for confirmation with timeout
            let wait_duration = Duration::from_secs(fee_bumping.initial_wait_seconds);

            // Record transaction submission time
            let submission_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            match tokio::time::timeout(
                wait_duration,
                pending_tx.with_required_confirmations(1).get_receipt(),
            )
            .await
            {
                Ok(Ok(receipt)) => {
                    let write_duration = write_start.elapsed();
                    info!("Transaction confirmed: 0x{:x}", tx_hash);

                    // Record successful contract write
                    ContractMetrics::record_contract_write(
                        feed_name,
                        &network_config.name,
                        true,
                        write_duration,
                        Some(&format!("0x{tx_hash:x}")),
                    );

                    // Record confirmation time
                    let confirmation_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    ContractMetrics::record_confirmation_time(
                        feed_name,
                        &network_config.name,
                        submission_time,
                        confirmation_time,
                    );

                    // Record gas metrics
                    GasMetrics::record_transaction(
                        feed_name,
                        &network_config.name,
                        &receipt,
                        gas_estimate.gas_limit,
                        &network_config.transaction_type,
                    );

                    // Log transaction if repository is available
                    if let Some(repo) = &tx_log_repo {
                        if let Err(e) = Self::log_transaction(
                            repo,
                            &tx_hash,
                            &receipt,
                            feed_name,
                            &network_config.name,
                            &gas_estimate,
                            &network_config.transaction_type,
                        )
                        .await
                        {
                            error!("Failed to log transaction: {}", e);
                        }
                    }

                    return Ok(receipt);
                }
                Ok(Err(e)) => {
                    error!("Transaction failed: {}", e);

                    // Record failed transaction
                    GasMetrics::record_failed_transaction(
                        feed_name,
                        &network_config.name,
                        gas_estimate.gas_limit,
                        gas_estimate.gas_price.or(gas_estimate.max_fee_per_gas),
                        &network_config.transaction_type,
                        &e.to_string(),
                    );

                    if attempt >= max_attempts {
                        return Err(anyhow::anyhow!(
                            "Transaction failed after {} attempts: {}",
                            attempt as u32,
                            e
                        ));
                    }
                }
                Err(_) => {
                    warn!(
                        "Transaction timed out after {} seconds: 0x{:x}",
                        wait_duration.as_secs(),
                        tx_hash
                    );
                    if attempt >= max_attempts {
                        return Err(anyhow::anyhow!(
                            "Transaction timed out after {} attempts",
                            attempt
                        ));
                    }
                }
            }

            // Bump fees for retry
            if fee_bumping.enabled && attempt < max_attempts {
                gas_estimate = gas_estimator.bump_fees(&gas_estimate, attempt);
                info!("Bumping fees for retry attempt {}", attempt + 1);
            }
        }
    }

    /// Log transaction details to the database
    async fn log_transaction(
        repo: &Arc<TransactionLogRepository>,
        tx_hash: &alloy::primitives::TxHash,
        receipt: &TransactionReceipt,
        feed_name: &str,
        network_name: &str,
        gas_estimate: &GasEstimate,
        tx_type: &str,
    ) -> Result<()> {
        let gas_used = receipt.gas_used;
        let gas_limit = gas_estimate.gas_limit;
        let efficiency_percent = (gas_used as f64 / gas_limit.to::<u128>() as f64) * 100.0;

        let gas_price_gwei = if let Some(price) = gas_estimate.gas_price {
            alloy::primitives::utils::format_units(price, "gwei")?.parse::<f64>()?
        } else if let Some(max_fee) = gas_estimate.max_fee_per_gas {
            alloy::primitives::utils::format_units(max_fee, "gwei")?.parse::<f64>()?
        } else {
            0.0
        };

        let total_cost_wei = U256::from(gas_used) * gas_estimate.gas_price.unwrap_or(U256::ZERO);

        let details = TransactionDetails {
            tx_hash: format!("0x{tx_hash:x}"),
            feed_name: feed_name.to_string(),
            network: network_name.to_string(),
            gas_limit: gas_limit.to::<u64>(),
            gas_used: gas_used as u64,
            gas_price_gwei,
            total_cost_wei: total_cost_wei.to::<u128>(),
            efficiency_percent,
            tx_type: tx_type.to_string(),
            status: if receipt.status() {
                "success"
            } else {
                "failed"
            }
            .to_string(),
            block_number: receipt.block_number.unwrap_or(0),
            error_message: None,
        };

        repo.save_transaction(details).await?;
        Ok(())
    }
}
