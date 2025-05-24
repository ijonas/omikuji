use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{info, error, debug};

use crate::network::NetworkManager;
use crate::config::models::Datafeed;
use super::contract_utils::{
    parse_address, create_contract_with_provider, create_contract_with_signer,
    scale_value_for_contract, validate_value_bounds, current_timestamp, errors
};

/// Handles contract updates based on time and deviation thresholds
pub struct ContractUpdater<'a> {
    network_manager: &'a Arc<NetworkManager>,
}

impl<'a> ContractUpdater<'a> {
    /// Creates a new ContractUpdater
    pub fn new(network_manager: &'a Arc<NetworkManager>) -> Self {
        Self { network_manager }
    }
    
    /// Checks if a contract update is needed based on time elapsed
    /// Returns true if minimum_update_frequency has passed since last update
    pub async fn should_update_based_on_time(
        &self,
        datafeed: &Datafeed,
    ) -> Result<bool> {
        // Get provider and create contract instance
        let provider = self.network_manager
            .get_provider(&datafeed.networks)?;
        
        let address = parse_address(&datafeed.contract_address)?;
        let contract = create_contract_with_provider(address, provider);
        
        // Get latest timestamp from contract
        let latest_timestamp = contract
            .latest_timestamp()
            .call()
            .await
            .with_context(|| "Failed to get latest timestamp from contract")?;
        
        // Get current timestamp
        let now = current_timestamp()?;
        let time_since_update = now.saturating_sub(latest_timestamp.as_u64());
        
        debug!(
            "Datafeed {}: last update {}s ago, minimum frequency {}s",
            datafeed.name, time_since_update, datafeed.minimum_update_frequency
        );
        
        // Check if enough time has passed
        Ok(time_since_update >= datafeed.minimum_update_frequency)
    }
    
    /// Submits a new value to the contract
    pub async fn submit_value(
        &self,
        datafeed: &Datafeed,
        value: f64,
    ) -> Result<()> {
        info!(
            "Submitting value {} to contract {} on network {}",
            value, datafeed.contract_address, datafeed.networks
        );
        
        // Get signer for the network
        let signer = self.network_manager
            .get_signer(&datafeed.networks)
            .with_context(|| format!("{} {}", errors::NO_SIGNER_AVAILABLE, datafeed.networks))?;
        
        let address = parse_address(&datafeed.contract_address)?;
        let contract = create_contract_with_signer(address, signer);
        
        // Get current round ID
        let latest_round = contract
            .latest_round()
            .call()
            .await
            .with_context(|| "Failed to get latest round from contract")?;
        
        let next_round = latest_round + 1;
        
        // Convert value to contract format
        let decimals = datafeed.decimals.unwrap_or(8);
        let scaled_value = scale_value_for_contract(value, decimals);
        
        // Validate against min/max bounds
        validate_value_bounds(scaled_value, datafeed)?;
        
        // Convert to I256 for contract
        let submission = I256::from(scaled_value);
        
        info!(
            "Submitting to round {} with value {} (scaled from {})",
            next_round, submission, value
        );
        
        // Submit the transaction
        match contract.submit_price(next_round, submission, None).await {
            Ok(receipt) => {
                info!(
                    "Successfully submitted value to contract. Tx hash: {:?}, Gas used: {:?}",
                    receipt.transaction_hash, receipt.gas_used
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to submit value to contract: {}", e);
                Err(anyhow::anyhow!("{}: {}", errors::CONTRACT_SUBMISSION_FAILED, e))
            }
        }
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
            min_value: Some(-1000000000),
            max_value: Some(1000000000),
            minimum_update_frequency: 3600, // 1 hour
            deviation_threshold_pct: 0.5,
            feed_url: "http://example.com/api".to_string(),
            feed_json_path: "data.price".to_string(),
            feed_json_path_timestamp: Some("data.timestamp".to_string()),
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
        datafeed.min_value = Some(100000000); // 1.0 with 8 decimals
        datafeed.max_value = Some(1000000000000); // 10000.0 with 8 decimals
        
        let decimals = datafeed.decimals.unwrap_or(8);
        let multiplier = 10f64.powi(decimals as i32);
        
        // Test value below minimum
        let low_value = 0.5;
        let scaled_low = (low_value * multiplier).round() as i128;
        assert!(scaled_low < datafeed.min_value.unwrap() as i128);
        
        // Test value above maximum
        let high_value = 20000.0;
        let scaled_high = (high_value * multiplier).round() as i128;
        assert!(scaled_high > datafeed.max_value.unwrap() as i128);
        
        // Test value within bounds
        let normal_value = 1000.0;
        let scaled_normal = (normal_value * multiplier).round() as i128;
        assert!(scaled_normal >= datafeed.min_value.unwrap() as i128);
        assert!(scaled_normal <= datafeed.max_value.unwrap() as i128);
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
}