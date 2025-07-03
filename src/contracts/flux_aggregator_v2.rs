//! Refactored FluxAggregator contract implementation using the common interaction pattern
//!
//! This demonstrates how to use the ContractInteraction abstraction to reduce code duplication

use crate::config::models::Network as NetworkConfig;
use crate::contracts::interaction::{ContractInteraction, ContractReader};
use crate::database::TransactionLogRepository;
use crate::gas_price::GasPriceManager;
use crate::utils::TransactionContext;
use alloy::{
    network::Ethereum,
    primitives::{Address, I256, U256},
    providers::Provider,
    rpc::types::TransactionReceipt,
    sol,
    sol_types::SolCall,
    transports::Transport,
};
use anyhow::Result;
use std::sync::Arc;

// Re-export the Solidity interface
sol! {
    #[sol(rpc)]
    interface IFluxAggregator {
        function latestAnswer() external view returns (int256);
        function latestTimestamp() external view returns (uint256);
        function latestRound() external view returns (uint256);
        function decimals() external view returns (uint8);
        function description() external view returns (string);
        function version() external view returns (uint256);
        function minSubmissionValue() external view returns (int256);
        function maxSubmissionValue() external view returns (int256);
        function submit(uint256 _roundId, int256 _submission) external;
    }
}

/// Parameters for submitting a price to the contract
pub struct SubmitPriceParams<'a> {
    pub round_id: U256,
    pub price: I256,
    pub network_config: &'a NetworkConfig,
    pub feed_name: &'a str,
    pub gas_limit: Option<u64>,
    pub tx_log_repo: Option<Arc<TransactionLogRepository>>,
    pub gas_price_manager: Option<&'a Arc<GasPriceManager>>,
}

/// Simplified FluxAggregator contract wrapper using common patterns
pub struct FluxAggregatorV2<T: Transport + Clone, P: Provider<T, Ethereum>> {
    address: Address,
    provider: Arc<P>,
    network_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Transport + Clone, P: Provider<T, Ethereum> + Clone> FluxAggregatorV2<T, P> {
    /// Create a new FluxAggregator contract instance
    pub fn new(address: Address, provider: Arc<P>, network_name: String) -> Self {
        Self {
            address,
            provider,
            network_name,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the latest answer from the contract
    pub async fn latest_answer(&self) -> Result<I256> {
        self.latest_answer_with_metrics(None).await
    }

    /// Get the latest answer with metrics tracking
    pub async fn latest_answer_with_metrics(&self, feed_name: Option<&str>) -> Result<I256> {
        let call = IFluxAggregator::latestAnswerCall {};
        let mut reader = ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        );

        if let Some(name) = feed_name {
            reader = reader.with_feed_name(name.to_string());
        }

        reader
            .call(call.abi_encode(), "latestAnswer", |bytes| {
                let decoded = IFluxAggregator::latestAnswerCall::abi_decode_returns(bytes, true)?;
                Ok(decoded._0)
            })
            .await
    }

    /// Get the latest timestamp
    pub async fn latest_timestamp(&self) -> Result<U256> {
        self.latest_timestamp_with_metrics(None).await
    }

    /// Get the latest timestamp with metrics tracking
    pub async fn latest_timestamp_with_metrics(&self, feed_name: Option<&str>) -> Result<U256> {
        let call = IFluxAggregator::latestTimestampCall {};
        let mut reader = ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        );

        if let Some(name) = feed_name {
            reader = reader.with_feed_name(name.to_string());
        }

        reader
            .call(call.abi_encode(), "latestTimestamp", |bytes| {
                let decoded =
                    IFluxAggregator::latestTimestampCall::abi_decode_returns(bytes, true)?;
                Ok(decoded._0)
            })
            .await
    }

    /// Get the latest round (no metrics version for backward compatibility)
    pub async fn latest_round(&self) -> Result<U256> {
        let call = IFluxAggregator::latestRoundCall {};
        ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        )
        .call(call.abi_encode(), "latestRound", |bytes| {
            let decoded = IFluxAggregator::latestRoundCall::abi_decode_returns(bytes, true)?;
            Ok(decoded._0)
        })
        .await
    }

    /// Get decimals
    pub async fn decimals(&self) -> Result<u8> {
        let call = IFluxAggregator::decimalsCall {};
        ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        )
        .call(call.abi_encode(), "decimals", |bytes| {
            let decoded = IFluxAggregator::decimalsCall::abi_decode_returns(bytes, true)?;
            Ok(decoded._0)
        })
        .await
    }

    /// Get min submission value
    pub async fn min_submission_value(&self) -> Result<I256> {
        let call = IFluxAggregator::minSubmissionValueCall {};
        ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        )
        .call(call.abi_encode(), "minSubmissionValue", |bytes| {
            let decoded = IFluxAggregator::minSubmissionValueCall::abi_decode_returns(bytes, true)?;
            Ok(decoded._0)
        })
        .await
    }

    /// Get max submission value
    pub async fn max_submission_value(&self) -> Result<I256> {
        let call = IFluxAggregator::maxSubmissionValueCall {};
        ContractReader::new(
            Arc::clone(&self.provider),
            self.address,
            self.network_name.clone(),
        )
        .call(call.abi_encode(), "maxSubmissionValue", |bytes| {
            let decoded = IFluxAggregator::maxSubmissionValueCall::abi_decode_returns(bytes, true)?;
            Ok(decoded._0)
        })
        .await
    }

    /// Submit a new price to the contract
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_price(
        &self,
        round_id: U256,
        price: I256,
        network_config: &NetworkConfig,
        feed_name: &str,
        tx_log_repo: Option<Arc<TransactionLogRepository>>,
        gas_price_manager: Option<&Arc<GasPriceManager>>,
    ) -> Result<TransactionReceipt> {
        let params = SubmitPriceParams {
            round_id,
            price,
            network_config,
            feed_name,
            gas_limit: None,
            tx_log_repo,
            gas_price_manager,
        };
        self.submit_price_with_params(params).await
    }

    /// Submit a price with custom gas limit
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_price_with_gas_limit(
        &self,
        round_id: U256,
        price: I256,
        network_config: &NetworkConfig,
        feed_name: &str,
        gas_limit: u64,
        tx_log_repo: Option<Arc<TransactionLogRepository>>,
        gas_price_manager: Option<&Arc<GasPriceManager>>,
    ) -> Result<TransactionReceipt> {
        let params = SubmitPriceParams {
            round_id,
            price,
            network_config,
            feed_name,
            gas_limit: Some(gas_limit),
            tx_log_repo,
            gas_price_manager,
        };
        self.submit_price_with_params(params).await
    }

    /// Submit a price using parameters struct
    pub async fn submit_price_with_params(
        &self,
        params: SubmitPriceParams<'_>,
    ) -> Result<TransactionReceipt> {
        let call = IFluxAggregator::submitCall {
            _roundId: params.round_id,
            _submission: params.price,
        };

        let context = TransactionContext::Datafeed {
            feed_name: params.feed_name.to_string(),
        };

        let interaction = ContractInteraction::new(
            Arc::clone(&self.provider),
            self.address,
            params.network_config.clone(),
        )
        .with_feed_name(params.feed_name.to_string());

        interaction
            .submit_transaction_with_handling(
                call.abi_encode(),
                context,
                params.gas_limit,
                params.tx_log_repo,
                params.gas_price_manager,
            )
            .await
    }
}

/// Factory function to create FluxAggregator instances
pub fn create_flux_aggregator<T, P>(
    address: Address,
    provider: Arc<P>,
    network_name: String,
) -> FluxAggregatorV2<T, P>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Clone,
{
    FluxAggregatorV2::new(address, provider, network_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn test_flux_aggregator_creation() {
        let provider = Arc::new(
            alloy::providers::ProviderBuilder::new()
                .on_http("http://localhost:8545".parse().unwrap()),
        );
        let address = address!("0000000000000000000000000000000000000000");

        let _aggregator = FluxAggregatorV2::new(address, provider, "test".to_string());
    }
}
