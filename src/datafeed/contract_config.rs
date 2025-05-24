use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tracing::info;

use crate::network::NetworkManager;
use super::contract_utils::{parse_address, create_contract_with_provider};

/// Configuration values read from a FluxAggregator contract
#[derive(Debug, Clone)]
pub struct ContractConfig {
    pub decimals: u8,
    pub min_value: i64,  // Store as i64 to match config model
    pub max_value: i64,  // Store as i64 to match config model
}

/// Reads configuration from FluxAggregator contracts
pub struct ContractConfigReader<'a> {
    network_manager: &'a Arc<NetworkManager>,
}

impl<'a> ContractConfigReader<'a> {
    /// Creates a new ContractConfigReader
    pub fn new(network_manager: &'a Arc<NetworkManager>) -> Self {
        Self { network_manager }
    }
    
    /// Reads configuration from a FluxAggregator contract
    /// 
    /// # Arguments
    /// * `network_name` - The network the contract is deployed on
    /// * `contract_address` - The address of the FluxAggregator contract
    /// 
    /// # Returns
    /// The contract configuration or an error
    pub async fn read_config(
        &self,
        network_name: &str,
        contract_address: &str,
    ) -> Result<ContractConfig> {
        info!(
            "Reading contract config from {} on network {}",
            contract_address, network_name
        );
        
        // Parse the contract address
        let address = parse_address(contract_address)?;
        
        // Get provider for the network
        let provider = self.network_manager
            .get_provider(network_name)
            .with_context(|| format!("Failed to get provider for network: {}", network_name))?;
        
        // Create contract instance
        let contract = create_contract_with_provider(address, provider);
        
        // Read decimals
        let decimals = contract
            .decimals()
            .call()
            .await
            .with_context(|| "Failed to read decimals from contract")?;
        
        // Read min submission value
        let min_value_i256 = contract
            .min_submission_value()
            .call()
            .await
            .with_context(|| "Failed to read minSubmissionValue from contract")?;
        
        // Read max submission value
        let max_value_i256 = contract
            .max_submission_value()
            .call()
            .await
            .with_context(|| "Failed to read maxSubmissionValue from contract")?;
        
        // Convert I256 to i64
        // Note: This could overflow if values are too large, but that's unlikely for price feeds
        let min_value = convert_i256_to_i64(min_value_i256)
            .with_context(|| "minSubmissionValue too large to fit in i64")?;
        
        let max_value = convert_i256_to_i64(max_value_i256)
            .with_context(|| "maxSubmissionValue too large to fit in i64")?;
        
        let config = ContractConfig {
            decimals,
            min_value,
            max_value,
        };
        
        info!(
            "Successfully read contract config: decimals={}, min_value={}, max_value={}",
            config.decimals, config.min_value, config.max_value
        );
        
        Ok(config)
    }
}

/// Converts an I256 to i64, returning an error if the value doesn't fit
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn convert_i256_to_i64(value: I256) -> Result<i64> {
    // Check if the value fits in i64
    let min_i64 = I256::from(i64::MIN);
    let max_i64 = I256::from(i64::MAX);
    
    if value < min_i64 || value > max_i64 {
        anyhow::bail!("Value {} is out of range for i64", value);
    }
    
    // For values that fit in i64, we can use try_into
    // which will handle the conversion safely
    value.try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert I256 to i64"))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper for conversion tests
    fn test_i256_conversion(input: i64, expected: i64) {
        let value = I256::from(input);
        let result = convert_i256_to_i64(value).unwrap();
        assert_eq!(result, expected);
    }
    
    // Helper for overflow tests
    fn test_i256_overflow(value: I256, error_contains: &str) {
        let result = convert_i256_to_i64(value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(error_contains));
    }
    
    #[test]
    fn test_convert_i256_to_i64_values() {
        test_i256_conversion(12345, 12345);
        test_i256_conversion(-12345, -12345);
        test_i256_conversion(i64::MAX, i64::MAX);
        test_i256_conversion(i64::MIN, i64::MIN);
    }
    
    #[test]
    fn test_convert_i256_to_i64_bounds() {
        // Test overflow
        test_i256_overflow(I256::from(i64::MAX) + I256::from(1), "out of range");
        
        // Test underflow
        test_i256_overflow(I256::from(i64::MIN) - I256::from(1), "out of range");
    }
    
    #[test]
    fn test_contract_config_struct() {
        let config = ContractConfig {
            decimals: 8,
            min_value: -1000000,
            max_value: 1000000,
        };
        
        assert_eq!(config.decimals, 8);
        assert_eq!(config.min_value, -1000000);
        assert_eq!(config.max_value, 1000000);
    }
}