use anyhow::{Context, Result};
use alloy::{
    primitives::{I256, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    network::{Ethereum, EthereumWallet},
    transports::http::{Client, Http},
};
use std::sync::Arc;
use tracing::{info, error, debug};
use url::Url;

use crate::network::NetworkManager;
use crate::config::models::{Datafeed, OmikujiConfig};
use crate::contracts::FluxAggregatorContract;
use crate::database::TransactionLogRepository;
use crate::metrics::FeedMetrics;
use super::contract_utils::{
    parse_address, create_contract_with_provider,
    scale_value_for_contract, validate_value_bounds, current_timestamp, 
    calculate_deviation_percentage, errors
};

/// Handles contract updates based on time and deviation thresholds
pub struct ContractUpdater<'a> {
    network_manager: &'a Arc<NetworkManager>,
    config: &'a OmikujiConfig,
    tx_log_repo: Option<Arc<TransactionLogRepository>>,
}

impl<'a> ContractUpdater<'a> {
    /// Creates a new ContractUpdater
    pub fn new(network_manager: &'a Arc<NetworkManager>, config: &'a OmikujiConfig) -> Self {
        Self { network_manager, config, tx_log_repo: None }
    }
    
    /// Creates a new ContractUpdater with transaction logging
    pub fn with_tx_logging(
        network_manager: &'a Arc<NetworkManager>, 
        config: &'a OmikujiConfig,
        tx_log_repo: Arc<TransactionLogRepository>
    ) -> Self {
        Self { network_manager, config, tx_log_repo: Some(tx_log_repo) }
    }
    
    /// Gets the network configuration for a datafeed
    fn get_network_config(&self, datafeed: &Datafeed) -> Result<&crate::config::models::Network> {
        self.config.networks
            .iter()
            .find(|n| n.name == datafeed.networks)
            .ok_or_else(|| anyhow::anyhow!("Network {} not found in configuration", datafeed.networks))
    }
    
    /// Gets a contract instance with provider for read operations
    async fn get_contract_for_read(&self, datafeed: &Datafeed) -> Result<FluxAggregatorContract<Http<Client>, RootProvider<Http<Client>>>> {
        let provider = self.network_manager
            .get_provider(&datafeed.networks)?;
        let address = parse_address(&datafeed.contract_address)?;
        Ok(create_contract_with_provider(address, provider.as_ref().clone()))
    }
    
    /// Creates a provider with signer for write operations
    async fn create_signer_provider(&self, network_name: &str) -> Result<impl Provider<Http<Client>, Ethereum> + Clone> {
        // Get the private key and RPC URL
        let private_key = self.network_manager
            .get_private_key(network_name)
            .with_context(|| format!("{} {}", errors::NO_SIGNER_AVAILABLE, network_name))?;
        
        let rpc_url = self.network_manager
            .get_rpc_url(network_name)?;
        
        // Parse the private key
        let signer = private_key
            .parse::<PrivateKeySigner>()
            .with_context(|| "Failed to parse private key as signer")?;
        
        let wallet = EthereumWallet::from(signer);
        
        // Create a provider with wallet
        let url = Url::parse(rpc_url)
            .with_context(|| format!("Failed to parse RPC URL: {}", rpc_url))?;
        
        let provider_with_wallet = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(url);
        
        Ok(provider_with_wallet)
    }
    
    /// Checks if a contract update is needed based on time elapsed
    /// Returns true if minimum_update_frequency has passed since last update
    pub async fn should_update_based_on_time(
        &self,
        datafeed: &Datafeed,
    ) -> Result<bool> {
        // Get contract instance
        let contract = self.get_contract_for_read(datafeed).await?;
        
        // Get latest timestamp from contract
        let latest_timestamp = contract
            .latest_timestamp()
            .await
            .with_context(|| "Failed to get latest timestamp from contract")?;
        
        // Get current timestamp
        let now = current_timestamp()?;
        let time_since_update = now.saturating_sub(latest_timestamp.to::<u64>());
        
        debug!(
            "Datafeed {}: last update {}s ago, minimum frequency {}s",
            datafeed.name, time_since_update, datafeed.minimum_update_frequency
        );
        
        // Check if enough time has passed
        Ok(time_since_update >= datafeed.minimum_update_frequency)
    }
    
    /// Checks if a contract update is needed based on value deviation
    /// Returns true if the new value deviates from the current on-chain value 
    /// by more than the configured threshold percentage
    pub async fn should_update_based_on_deviation(
        &self,
        datafeed: &Datafeed,
        new_value: f64,
    ) -> Result<bool> {
        // Get contract instance
        let contract = self.get_contract_for_read(datafeed).await?;
        
        // Get latest answer from contract
        let latest_answer = match contract.latest_answer().await {
            Ok(answer) => answer,
            Err(e) => {
                error!(
                    "Failed to get latest answer from contract for datafeed {}: {}. Skipping deviation check.",
                    datafeed.name, e
                );
                return Ok(false);
            }
        };
        
        // Scale the new value to match contract format
        let decimals = datafeed.decimals.unwrap_or(8);
        let scaled_new_value = scale_value_for_contract(new_value, decimals);
        
        // Convert I256 to i128 for comparison
        let current_value = latest_answer.try_into()
            .context("Failed to convert latest answer to i128")?;
        
        // Calculate deviation percentage
        let deviation = calculate_deviation_percentage(current_value, scaled_new_value);
        
        debug!(
            "Datafeed {}: current value {}, new value {} (scaled), deviation {}%",
            datafeed.name, current_value, scaled_new_value, deviation
        );
        
        // Check if deviation exceeds threshold
        let exceeds_threshold = deviation > datafeed.deviation_threshold_pct;
        
        if exceeds_threshold {
            info!(
                "Datafeed {}: deviation {}% exceeds threshold {}%",
                datafeed.name, deviation, datafeed.deviation_threshold_pct
            );
        }
        
        Ok(exceeds_threshold)
    }
    
    /// Checks if an update is needed based on either time or deviation thresholds
    /// Returns (should_update, reason) where reason describes what triggered the update
    pub async fn check_update_needed(
        &self,
        datafeed: &Datafeed,
        new_value: f64,
    ) -> Result<(bool, &'static str)> {
        // Check both conditions
        let time_check = self.should_update_based_on_time(datafeed).await?;
        let deviation_check = self.should_update_based_on_deviation(datafeed, new_value).await?;
        
        // Determine if update is needed and why
        match (time_check, deviation_check) {
            (true, true) => Ok((true, "both time and deviation thresholds")),
            (true, false) => Ok((true, "time threshold")),
            (false, true) => Ok((true, "deviation threshold")),
            (false, false) => Ok((false, "")),
        }
    }
    
    /// Submits a new value to the contract
    pub async fn submit_value(
        &self,
        datafeed: &Datafeed,
        value: f64,
        dashboard: Option<Arc<tokio::sync::RwLock<crate::tui::DashboardState>>>,
    ) -> Result<()> {
        info!(
            "Submitting value {} to contract {} on network {}",
            value, datafeed.contract_address, datafeed.networks
        );
        
        // Create provider with signer
        let provider = self.create_signer_provider(&datafeed.networks).await?;
        
        // Create contract instance with the signing provider
        let address = parse_address(&datafeed.contract_address)?;
        let contract = create_contract_with_provider(address, provider);
        
        // Get current round ID
        let latest_round = contract
            .latest_round()
            .await
            .with_context(|| "Failed to get latest round from contract")?;
        
        let next_round = latest_round + U256::from(1);
        
        // Convert value to contract format
        let decimals = datafeed.decimals.unwrap_or(8);
        let scaled_value = scale_value_for_contract(value, decimals);
        
        // Validate against min/max bounds
        validate_value_bounds(scaled_value, datafeed)?;
        
        // Convert to I256 for contract
        let submission = I256::try_from(scaled_value)
            .context("Failed to convert scaled value to I256")?;
        
        info!(
            "Submitting to round {} with value {} (scaled from {})",
            next_round, submission, value
        );
        
        // Get network configuration for gas settings
        let network_config = self.get_network_config(datafeed)?;
        
        // Get wallet address for gas estimation
        let wallet_address = self.network_manager
            .get_wallet_address(&datafeed.networks)
            .ok(); // It's optional, so we use ok() to convert Result to Option
        
        // Submit the transaction with gas estimation
        match contract.submit_price_with_gas_estimation(
            next_round, 
            submission, 
            network_config,
            &datafeed.name,
            self.tx_log_repo.clone(),
            wallet_address,
        ).await {
            Ok(receipt) => {
                info!(
                    "Successfully submitted value to contract. Tx hash: 0x{:x}, Gas used: {}",
                    receipt.transaction_hash, receipt.gas_used
                );
                
                // Record contract update in metrics
                FeedMetrics::record_contract_update(&datafeed.name, &datafeed.networks);
                
                // --- Update dashboard with tx cost and count ---
                if let Some(dash) = dashboard {
                    // Calculate cost in ETH
                    let gas_used = receipt.gas_used;
                    let gas_price = receipt.effective_gas_price;
                    let cost_eth = (gas_used as f64) * (gas_price as f64) / 1e18;
                    crate::tui::update::set_last_tx_cost(&dash, cost_eth).await;
                    // Increment tx_count
                    let mut dash_w = dash.write().await;
                    dash_w.metrics.tx_count += 1;
                }
                
                Ok(())
            }
            Err(e) => {
                error!("Failed to submit value to contract: {}", e);
                Err(anyhow::anyhow!("{}: {}", errors::CONTRACT_SUBMISSION_FAILED, e))
            }
        }
    }
    
    /// Read current contract state and update metrics
    pub async fn update_contract_metrics(
        &self,
        datafeed: &Datafeed,
        feed_value: f64,
    ) -> Result<()> {
        // Get contract instance
        let contract = self.get_contract_for_read(datafeed).await?;
        
        // Get latest answer from contract
        let latest_answer = contract
            .latest_answer()
            .await
            .with_context(|| "Failed to get latest answer from contract")?;
        
        // Get latest timestamp
        let latest_timestamp = contract
            .latest_timestamp()
            .await
            .with_context(|| "Failed to get latest timestamp from contract")?;
        
        // Get latest round
        let latest_round = contract
            .latest_round()
            .await
            .with_context(|| "Failed to get latest round from contract")?;
        
        // Convert contract value from scaled integer to float
        let decimals = datafeed.decimals.unwrap_or(8);
        let divisor = 10f64.powi(decimals as i32);
        let answer_i128: i128 = latest_answer.try_into()
            .context("Failed to convert latest answer to i128")?;
        let contract_value = answer_i128 as f64 / divisor;
        
        // Update metrics
        FeedMetrics::set_contract_value(
            &datafeed.name,
            &datafeed.networks,
            contract_value,
            latest_round.to::<u64>(),
            latest_timestamp.to::<u64>(),
        );
        
        // Calculate and update deviation
        FeedMetrics::update_deviation(
            &datafeed.name,
            &datafeed.networks,
            feed_value,
            contract_value,
        );
        
        debug!(
            "Updated contract metrics for {}: value={}, round={}, timestamp={}",
            datafeed.name, contract_value, latest_round, latest_timestamp
        );
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::Datafeed;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_datafeed() -> Datafeed {
        Datafeed {
            name: "test_feed".to_string(),
            networks: "test".to_string(),
            check_frequency: 60,
            contract_address: "0x1234567890123456789012345678901234567890".to_string(),
            contract_type: "fluxmon".to_string(),
            read_contract_config: false,
            decimals: Some(8),
            min_value: Some(I256::try_from(-1000000000).unwrap()),
            max_value: Some(I256::try_from(1000000000).unwrap()),
            minimum_update_frequency: 3600, // 1 hour
            deviation_threshold_pct: 0.5,
            feed_url: "http://example.com/api".to_string(),
            feed_json_path: "data.price".to_string(),
            feed_json_path_timestamp: Some("data.timestamp".to_string()),
            data_retention_days: 7,
        }
    }

    #[test]
    fn test_value_scaling_with_decimals() {
        let datafeed = create_test_datafeed();
        let value = 2557.96;
        let decimals = datafeed.decimals.unwrap_or(8);
        let scaled_value = scale_value_for_contract(value, decimals);
        
        assert_eq!(scaled_value, 255796000000);
    }

    #[test]
    fn test_value_bounds_validation() {
        let mut datafeed = create_test_datafeed();
        datafeed.min_value = Some(I256::try_from(100000000).unwrap()); // 1.0 with 8 decimals
        datafeed.max_value = Some(I256::try_from(1000000000000i64).unwrap()); // 10000.0 with 8 decimals
        
        let decimals = datafeed.decimals.unwrap_or(8);
        let multiplier = 10f64.powi(decimals as i32);
        
        // Test value below minimum
        let low_value = 0.5;
        let scaled_low = (low_value * multiplier).round() as i128;
        assert!(I256::try_from(scaled_low).unwrap() < datafeed.min_value.unwrap());
        
        // Test value above maximum
        let high_value = 20000.0;
        let scaled_high = (high_value * multiplier).round() as i128;
        assert!(I256::try_from(scaled_high).unwrap() > datafeed.max_value.unwrap());
        
        // Test value within bounds
        let normal_value = 1000.0;
        let scaled_normal = (normal_value * multiplier).round() as i128;
        assert!(I256::try_from(scaled_normal).unwrap() >= datafeed.min_value.unwrap());
        assert!(I256::try_from(scaled_normal).unwrap() <= datafeed.max_value.unwrap());
    }

    #[test]
    fn test_time_difference_calculation() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Simulate last update 30 minutes ago
        let last_update = now - 1800;
        let time_since_update = now.saturating_sub(last_update);
        
        assert_eq!(time_since_update, 1800);
        
        // Test with 1 hour minimum update frequency
        let minimum_frequency = 3600u64;
        assert!(time_since_update < minimum_frequency); // Should not update
        
        // Simulate last update 2 hours ago
        let old_update = now - 7200;
        let time_since_old = now.saturating_sub(old_update);
        assert!(time_since_old >= minimum_frequency); // Should update
    }

    #[test]
    fn test_negative_value_scaling() {
        let datafeed = create_test_datafeed();
        let value = -123.45;
        let decimals = datafeed.decimals.unwrap_or(8);
        let multiplier = 10f64.powi(decimals as i32);
        let scaled_value = (value * multiplier).round() as i128;
        
        assert_eq!(scaled_value, -12345000000);
    }

    #[test]
    fn test_zero_value_scaling() {
        let datafeed = create_test_datafeed();
        let value = 0.0;
        let decimals = datafeed.decimals.unwrap_or(8);
        let multiplier = 10f64.powi(decimals as i32);
        let scaled_value = (value * multiplier).round() as i128;
        
        assert_eq!(scaled_value, 0);
    }

    #[test]
    fn test_different_decimal_scales() {
        let mut datafeed = create_test_datafeed();
        let value = 100.0;
        
        // Test with 18 decimals (common for ETH)
        datafeed.decimals = Some(18);
        let multiplier = 10f64.powi(18);
        let scaled_18 = (value * multiplier).round() as i128;
        assert_eq!(scaled_18, 100_000_000_000_000_000_000);
        
        // Test with 6 decimals (common for USDC)
        datafeed.decimals = Some(6);
        let multiplier = 10f64.powi(6);
        let scaled_6 = (value * multiplier).round() as i128;
        assert_eq!(scaled_6, 100_000_000);
        
        // Test with 0 decimals
        datafeed.decimals = Some(0);
        let multiplier = 10f64.powi(0);
        let scaled_0 = (value * multiplier).round() as i128;
        assert_eq!(scaled_0, 100);
    }

    #[test]
    fn test_rounding_behavior() {
        let datafeed = create_test_datafeed();
        let decimals = datafeed.decimals.unwrap_or(8);
        let multiplier = 10f64.powi(decimals as i32);
        
        // Test rounding up
        let value_up = 123.456789;
        let scaled_up = (value_up * multiplier).round() as i128;
        assert_eq!(scaled_up, 12345678900);
        
        // Test rounding down
        let value_down = 123.454321;
        let scaled_down = (value_down * multiplier).round() as i128;
        assert_eq!(scaled_down, 12345432100);
    }

    #[test]
    fn test_minimum_update_frequency_edge_cases() {
        let datafeed = create_test_datafeed();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Test exact minimum frequency
        let last_update_exact = now - datafeed.minimum_update_frequency;
        let time_since_exact = now.saturating_sub(last_update_exact);
        assert_eq!(time_since_exact, datafeed.minimum_update_frequency);
        assert!(time_since_exact >= datafeed.minimum_update_frequency); // Should update
        
        // Test one second before minimum frequency
        let last_update_just_before = now - (datafeed.minimum_update_frequency - 1);
        let time_since_just_before = now.saturating_sub(last_update_just_before);
        assert_eq!(time_since_just_before, datafeed.minimum_update_frequency - 1);
        assert!(time_since_just_before < datafeed.minimum_update_frequency); // Should not update
    }

    #[test]
    fn test_deviation_threshold_calculations() {
        let datafeed = create_test_datafeed(); // has 0.5% threshold
        let decimals = datafeed.decimals.unwrap_or(8);
        
        // Current on-chain value: $100.00 (scaled)
        let current_value: i128 = 10000000000; // 100 * 10^8
        
        // Test value with 0.4% deviation (below threshold)
        let small_change = 100.4;
        let scaled_small = scale_value_for_contract(small_change, decimals);
        let small_deviation = calculate_deviation_percentage(current_value, scaled_small);
        assert!(small_deviation < datafeed.deviation_threshold_pct);
        
        // Test value with 0.6% deviation (above threshold)
        let large_change = 100.6;
        let scaled_large = scale_value_for_contract(large_change, decimals);
        let large_deviation = calculate_deviation_percentage(current_value, scaled_large);
        assert!(large_deviation > datafeed.deviation_threshold_pct);
        
        // Test exact threshold (0.5%)
        let exact_change = 100.5;
        let scaled_exact = scale_value_for_contract(exact_change, decimals);
        let exact_deviation = calculate_deviation_percentage(current_value, scaled_exact);
        assert_eq!(exact_deviation, datafeed.deviation_threshold_pct);
    }
}